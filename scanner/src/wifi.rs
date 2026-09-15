use anyhow::Result;
use esp_idf_svc::eventloop::{EspSystemEventLoop, EspSystemSubscription};
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi, WifiEvent};

pub fn connect(
    modem: Modem<'static>,
) -> Result<(
    EspSystemSubscription<'static>,
    BlockingWifi<EspWifi<'static>>,
)> {
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(modem, sys_loop.clone(), Some(nvs))?,
        sys_loop.clone(),
    )?;
    let ssid = "naroah";
    let password = "59aaf169d335c5d29c92457b9f89882bc986c1e89d1ee7814e24d22f7fa8c476";
    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().unwrap(),
        password: password.try_into().unwrap(),
        ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;
    wifi.start()?;
    esp_idf_svc::sys::esp!(unsafe {
        esp_idf_svc::sys::esp_wifi_set_ps(esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
    })?;
    esp_idf_svc::sys::esp!(unsafe {
        esp_idf_svc::sys::esp_wifi_set_bandwidth(
            esp_idf_svc::sys::wifi_interface_t_WIFI_IF_STA,
            esp_idf_svc::sys::wifi_bandwidth_t_WIFI_BW_HT20,
        )
    })?;

    let subscription = sys_loop.subscribe::<WifiEvent, _>(|event| {
        if let WifiEvent::StaDisconnected(info) = event {
            log::warn!(
                "WiFi disconnected: reason={}, rssi={}, bssid={:02x?}. Reconnecting...",
                info.reason(),
                info.rssi(),
                info.bssid()
            );
            if let Err(err) =
                esp_idf_svc::sys::esp!(unsafe { esp_idf_svc::sys::esp_wifi_connect() })
            {
                log::warn!("WiFi reconnect request failed: {:?}", err);
            }
        }
    })?;

    wifi.wifi_mut().connect()?;
    wifi.ip_wait_while(|| wifi.is_up().map(|up| !up), None)?;
    log::info!("WiFi connected");
    Ok((subscription, wifi))
}
