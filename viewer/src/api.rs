use anyhow::Result;
use embedded_svc::{http::client::Client, utils::io};
use esp_idf_svc::{
    http::client::{Configuration as HttpConfiguration, EspHttpConnection},
    io::Write,
};
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::HashMap;

const URL: &str = "https://aws.naroah.top/cheat/";

#[derive(Deserialize, Debug)]
pub struct Word {
    pub answer: Vec<String>,
    pub english: Vec<String>,
    pub math: u8,
}
#[derive(Deserialize, Debug)]
pub struct Answer {
    pub single_choice: Vec<String>,
    pub multiple_choice: Vec<String>,
    pub binary_choice: Vec<bool>,
    pub non_choice: Vec<Word>,
}
pub type Pages = Vec<u8>;
pub type Missing = HashMap<String, Vec<u8>>;
pub type Formula = Vec<u8>;
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
    fn get_bytes(&mut self, url: &str) -> Result<Vec<u8>> {
        let url = format!("{}{}", URL, url);
        let request = self.client.get(&url)?;
        let mut response = request.submit()?;

        let mut buf = [0u8; 1024];
        let bytes_read = io::try_read_full(&mut response, &mut buf).map_err(|e| e.0)?;
        Ok(buf[0..bytes_read].to_vec())
    }
    fn get_request<T: DeserializeOwned>(&mut self, url: &str) -> Result<T> {
        let bytes = self.get_bytes(url)?;
        let result: T = serde_json::from_slice(&bytes)?;
        Ok(result)
    }
    pub fn get_pages(&mut self) -> Result<Pages> {
        self.get_request::<Pages>("pages")
    }
    pub fn get_missing(&mut self) -> Result<Missing> {
        self.get_request::<Missing>("missing")
    }
    pub fn get_answer(&mut self) -> Result<Option<Answer>> {
        let bytes = self.get_bytes("answer")?;
        let body = std::str::from_utf8(&bytes)?.trim();
        if body.is_empty()
            || body.eq_ignore_ascii_case("null")
            || body.trim_matches('"').eq_ignore_ascii_case("none")
        {
            return Ok(None);
        }
        let answer = serde_json::from_slice(&bytes)?;
        Ok(Some(answer))
    }
    fn post_request(&mut self, url: &str) -> Result<()> {
        let url = format!("{}{}", URL, url);
        let headers = [("Content-Length", "0")];
        let request = self.client.post(&url, &headers)?;
        let mut response = request.submit()?;
        let mut buf = [0u8; 128];
        loop {
            let n = response.read(&mut buf)?;
            if n == 0 {
                break;
            }
        }
        Ok(())
    }
    pub fn reset(&mut self) -> Result<()> {
        self.post_request("reset")
    }
    pub fn upload(&mut self) -> Result<()> {
        self.post_request("upload")
    }
    pub fn get_formula(&mut self, position: (u8, u8)) -> Result<Formula> {
        let url = format!("{}{}", URL, "formula");
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
        let mut image_data = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            let n = response.read(&mut buf)?;
            if n == 0 {
                break;
            }
            image_data.extend_from_slice(&buf[0..n]);
        }
        Ok(image_data)
    }
}
