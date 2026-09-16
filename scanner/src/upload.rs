use anyhow::{bail, Result};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection, Method};
use serde::Deserialize;
use std::time::Duration;

const MAX_RESPONSE_SIZE: usize = 4096;

#[derive(Debug, Deserialize)]
pub struct PreCheckResponse {
    pub accepted: bool,
    pub page: Option<u32>,
    pub variance: Option<f64>,
    pub reject_reason: Option<String>,
}

pub fn image(image_data: Vec<u8>) -> Result<PreCheckResponse> {
    let url = "https://aws.naroah.top/cheat/pre-check";
    let config = Configuration {
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
        timeout: Some(Duration::from_secs(30)),
        buffer_size: Some(8192),
        buffer_size_tx: Some(8192),
        ..Default::default()
    };
    let boundary = "RustEsp32Boundary";
    let file_name = "esp32_photo.jpg";
    let header = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: image/jpeg\r\n\r\n",
        boundary, file_name
    );
    let footer = format!("\r\n--{}--\r\n", boundary);
    let total_len = header.len() + image_data.len() + footer.len();
    let content_length = total_len.to_string();
    let content_type = format!("multipart/form-data; boundary={}", boundary);
    let headers = [
        ("Content-Type", content_type.as_str()),
        ("Content-Length", content_length.as_str()),
    ];
    let chunk_size = 8192;
    let mut connection = EspHttpConnection::new(&config)?;

    connection.initiate_request(Method::Post, url, &headers)?;
    connection.write(header.as_bytes())?;
    for chunk in image_data.chunks(chunk_size) {
        connection.write(chunk)?;
    }
    connection.write(footer.as_bytes())?;
    connection.initiate_response()?;

    let status = connection.status();
    if (200..300).contains(&status) {
        log::info!(
            "HTTP image upload successful: endpoint={}, status={}",
            url,
            status
        );
        let mut buffer = [0u8; 512];
        let mut response = Vec::new();
        loop {
            let bytes_read = connection.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            if response.len() + bytes_read > MAX_RESPONSE_SIZE {
                bail!("Pre-check response exceeded {} bytes", MAX_RESPONSE_SIZE);
            }
            response.extend_from_slice(&buffer[..bytes_read]);
        }
        Ok(serde_json::from_slice(&response)?)
    } else {
        bail!("Server returned error for {}: {}", url, status)
    }
}
