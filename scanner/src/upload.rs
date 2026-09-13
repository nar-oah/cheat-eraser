use anyhow::{bail, Result};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection, Method};
use std::time::Duration;

pub fn image(image_data: Vec<u8>) -> Result<()> {
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
        log::info!("Upload Success! Status: {}", status);
        let mut buffer = [0u8; 512];
        let bytes_read = connection.read(&mut buffer)?;
        let response_text = std::str::from_utf8(&buffer[..bytes_read])?.to_string();
        log::info!("uuid:{}", response_text);
        Ok(())
    } else {
        bail!("Server returned error: {}", status)
    }
}
