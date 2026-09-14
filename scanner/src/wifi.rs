use anyhow::Result;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};

pub fn connect(modem: Modem<'static>) -> Result<BlockingWifi<EspWifi<'static>>> {
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sys_loop.clone(), Some(nvs))?, sys_loop)?;
    let ssid = "naroah";
    let password = "Ylds0601";
    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().unwrap(),
        password: password.try_into().unwrap(),
        ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;
    wifi.start()?;
    loop {
        match wifi.connect().and_then(|_| wifi.wait_netif_up()) {
            Ok(()) => break,
            Err(err) => {
                log::warn!("WiFi connection failed: {:?}. Retrying...", err);
                let _ = wifi.disconnect();
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }
    log::info!("WiFi connected");
    Ok(wifi)
}
