use crate::api_core::{check_status, parse_answer, parse_scanner_status, read_bounded};
pub use crate::api_core::{Answer, AnswerState};
use anyhow::{bail, Context, Result};
use embedded_svc::http::client::Client;
use esp_idf_svc::{
    http::client::{Configuration as HttpConfiguration, EspHttpConnection},
    io::Write,
};
use serde::de::DeserializeOwned;
use std::collections::HashMap;

const URL: &str = "https://aws.naroah.top/cheat/";

pub type Pages = Vec<u8>;
pub type Missing = HashMap<String, Vec<u8>>;
pub type Formula = Option<Vec<u8>>;

pub struct ApiClient {
    client: Client<EspHttpConnection>,
}

impl ApiClient {
    pub fn new() -> Result<Self> {
        let config = HttpConfiguration {
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            ..Default::default()
        };
        let client = Client::wrap(EspHttpConnection::new(&config)?);
        Ok(Self { client })
    }
    fn get_bytes(&mut self, endpoint: &str) -> Result<Vec<u8>> {
        let url = format!("{}{}", URL, endpoint);
        let request = self.client.get(&url)?;
        let mut response = request.submit()?;
        check_status(endpoint, response.status())?;
        read_bounded(endpoint, |buf| response.read(buf))
    }

    fn get_request<T: DeserializeOwned>(&mut self, endpoint: &str) -> Result<T> {
        let bytes = self.get_bytes(endpoint)?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("Invalid JSON from HTTP endpoint /{endpoint}"))
    }
    pub fn get_pages(&mut self) -> Result<Pages> {
        self.get_request::<Pages>("pages")
    }
    pub fn get_missing(&mut self) -> Result<Missing> {
        self.get_request::<Missing>("missing")
    }
    pub fn get_answer(&mut self) -> Result<AnswerState> {
        let bytes = self.get_bytes("answer")?;
        parse_answer(&bytes)
    }
    pub fn get_scanner_status(&mut self) -> Result<bool> {
        let bytes = self.get_bytes("scanner/status")?;
        parse_scanner_status(&bytes)
    }

    fn post_request(&mut self, endpoint: &str) -> Result<()> {
        let url = format!("{}{}", URL, endpoint);
        let headers = [("Content-Length", "0")];
        let request = self.client.post(&url, &headers)?;
        let mut response = request.submit()?;
        check_status(endpoint, response.status())?;
        read_bounded(endpoint, |buf| response.read(buf))?;
        Ok(())
    }
    pub fn reset(&mut self) -> Result<()> {
        self.post_request("reset")
    }
    pub fn upload(&mut self) -> Result<()> {
        self.post_request("upload")
    }

    pub fn get_formula(&mut self, position: (u8, u8)) -> Result<Formula> {
        let endpoint = "formula";
        let url = format!("{}{}", URL, endpoint);
        let body = serde_json::to_vec(&position)?;
        let body_len = body.len().to_string();
        let headers = [
            ("Content-Type", "application/json"),
            ("Content-Length", &body_len),
        ];
        let mut request = self.client.post(&url, &headers)?;
        request.write_all(&body)?;
        request.flush()?;
        let mut response = request.submit()?;
        let status = response.status();
        if status == 204 {
            return Ok(None);
        }
        check_status(endpoint, status)?;
        let content_type = response.header("Content-Type").unwrap_or_default();
        if !content_type
            .split(';')
            .next()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("image/bmp"))
        {
            bail!(
                "HTTP endpoint /{endpoint} returned status {status} with content type {content_type:?}"
            )
        }
        let image_data = read_bounded(endpoint, |buf| response.read(buf))?;
        if image_data.is_empty() {
            bail!("HTTP endpoint /{endpoint} returned status {status} with an empty BMP")
        }
        Ok(Some(image_data))
    }
}
