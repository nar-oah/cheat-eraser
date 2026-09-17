use anyhow::{bail, Context, Result};
use esp_idf_svc::eventloop::{EspSystemEventLoop, EspSystemSubscription};
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::sys;
use esp_idf_svc::wifi::{
    AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi, ScanMethod,
    ScanSortMethod, WifiEvent,
};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(5);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(250);
const DRIVER_RESET_DELAY: Duration = Duration::from_millis(250);
const MIN_RETRY_DELAY: Duration = Duration::from_millis(500);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(8);

pub struct WifiConnection {
    _subscription: EspSystemSubscription<'static>,
    _worker: JoinHandle<()>,
}

pub fn connect(modem: Modem<'static>) -> Result<WifiConnection> {
    let sys_loop = EspSystemEventLoop::take()?;
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(modem, sys_loop.clone(), None)?,
        sys_loop.clone(),
    )?;
    let wifi_configuration = Configuration::Client(ClientConfiguration {
        ssid: "naroah".try_into().unwrap(),
        password: "Ylds0601".try_into().unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        scan_method: ScanMethod::CompleteScan(ScanSortMethod::Signal),
        ..Default::default()
    });

    sys::esp!(unsafe { sys::esp_wifi_set_country_code(b"CN\0".as_ptr().cast(), false) })?;
    sys::esp!(unsafe { sys::esp_wifi_set_storage(sys::wifi_storage_t_WIFI_STORAGE_RAM) })?;
    wifi.set_configuration(&wifi_configuration)?;
    wifi.start()?;
    configure_radio()?;

    let (disconnect_sender, disconnect_receiver) = sync_channel(1);
    let subscription = subscribe(&sys_loop, disconnect_sender)?;

    let (ready_sender, ready_receiver) = sync_channel(1);
    let worker = thread::Builder::new()
        .name("wifi-reconnect".into())
        .stack_size(8 * 1024)
        .spawn(move || reconnect_loop(wifi, disconnect_receiver, ready_sender))
        .context("Failed to start WiFi reconnect worker")?;
    ready_receiver
        .recv()
        .context("WiFi reconnect worker stopped before obtaining an IP address")?;

    Ok(WifiConnection {
        _subscription: subscription,
        _worker: worker,
    })
}

fn configure_radio() -> Result<()> {
    sys::esp!(unsafe { sys::esp_wifi_set_ps(sys::wifi_ps_type_t_WIFI_PS_NONE) })?;
    sys::esp!(unsafe {
        sys::esp_wifi_set_protocol(
            sys::wifi_interface_t_WIFI_IF_STA,
            (sys::WIFI_PROTOCOL_11B | sys::WIFI_PROTOCOL_11G | sys::WIFI_PROTOCOL_11N) as u8,
        )
    })?;
    sys::esp!(unsafe {
        sys::esp_wifi_set_bandwidth(
            sys::wifi_interface_t_WIFI_IF_STA,
            sys::wifi_bandwidth_t_WIFI_BW_HT20,
        )
    })?;
    Ok(())
}

fn subscribe(
    sys_loop: &EspSystemEventLoop,
    disconnect_sender: SyncSender<()>,
) -> Result<EspSystemSubscription<'static>> {
    Ok(
        sys_loop.subscribe::<WifiEvent, _>(move |event| match event {
            WifiEvent::StaConnected(info) => log::info!(
                "WiFi associated: channel={}, auth={:?}, bssid={:02x?}",
                info.channel(),
                info.authmode(),
                info.bssid()
            ),
            WifiEvent::StaDisconnected(info) => {
                log::warn!(
                    "WiFi disconnected: reason={}, rssi={}, bssid={:02x?}",
                    info.reason(),
                    info.rssi(),
                    info.bssid()
                );
                let _ = disconnect_sender.try_send(());
            }
            _ => {}
        })?,
    )
}

fn reconnect_loop(
    mut wifi: BlockingWifi<EspWifi<'static>>,
    disconnects: Receiver<()>,
    ready: SyncSender<()>,
) {
    let mut ready = Some(ready);
    let mut retry_delay = MIN_RETRY_DELAY;
    let mut attempt = 0_u32;

    loop {
        if wifi.is_up().unwrap_or(false) {
            match disconnects.recv_timeout(HEALTH_CHECK_INTERVAL) {
                Ok(()) | Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    thread::sleep(HEALTH_CHECK_INTERVAL);
                    continue;
                }
            }
        }

        attempt = attempt.saturating_add(1);
        log::info!("WiFi connection attempt {attempt}");
        match connect_once(&mut wifi, &disconnects) {
            Ok(()) => {
                log::info!("WiFi connected and obtained an IP address");
                retry_delay = MIN_RETRY_DELAY;
                attempt = 0;
                if let Some(sender) = ready.take() {
                    let _ = sender.send(());
                }
            }
            Err(err) => {
                log::warn!(
                    "WiFi connection attempt {attempt} failed: {err:?}; retrying in {retry_delay:?}"
                );
                if let Err(reset_err) = restart_wifi(&mut wifi) {
                    log::warn!("Failed to reset WiFi driver: {reset_err:?}");
                }
                thread::sleep(retry_delay);
                retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY);
            }
        }
    }
}

fn restart_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> Result<()> {
    wifi.stop().context("failed to stop WiFi driver")?;
    thread::sleep(DRIVER_RESET_DELAY);
    wifi.start().context("failed to restart WiFi driver")?;
    configure_radio().context("failed to restore WiFi radio settings")
}

fn connect_once(
    wifi: &mut BlockingWifi<EspWifi<'static>>,
    disconnects: &Receiver<()>,
) -> Result<()> {
    while disconnects.try_recv().is_ok() {}
    wifi.wifi_mut().connect()?;
    let deadline = Instant::now() + CONNECT_TIMEOUT;

    loop {
        if wifi.is_up()? {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!("timed out waiting for DHCP");
        }
        let wait = STATUS_POLL_INTERVAL.min(remaining);
        match disconnects.recv_timeout(wait) {
            Ok(()) => bail!("access point rejected or lost the connection"),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => thread::sleep(wait),
        }
    }
}
