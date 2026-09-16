use anyhow::{bail, Result};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection, Method};
use std::time::Duration;

const STATUS_URL: &str = "https://aws.naroah.top/cheat/scanner/status";

pub fn update(ready: bool) -> Result<()> {
    let body: &[u8] = if ready {
        br#"{"ready":true}"#
    } else {
        br#"{"ready":false}"#
    };
    let content_length = body.len().to_string();
    let headers = [
        ("Content-Type", "application/json"),
        ("Content-Length", content_length.as_str()),
    ];
    let config = Configuration {
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
        timeout: Some(Duration::from_secs(5)),
        ..Default::default()
    };
    let mut connection = EspHttpConnection::new(&config)?;

    connection.initiate_request(Method::Post, STATUS_URL, &headers)?;
    connection.write(body)?;
    connection.initiate_response()?;

    let response_status = connection.status();
    if !(200..300).contains(&response_status) {
        bail!("Server returned scanner status error: {}", response_status);
    }
    Ok(())
}
