use anyhow::{anyhow, bail, Context, Result};
use embedded_svc::http::client::Client;
use esp_idf_svc::{
    http::client::{Configuration as HttpConfiguration, EspHttpConnection},
    io::Write,
};
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::HashMap;
use std::fmt::Display;

const URL: &str = "https://aws.naroah.top/cheat/";
const MAX_RESPONSE_SIZE: usize = 256 * 1024;
const RESPONSE_CHUNK_SIZE: usize = 1024;

#[derive(Deserialize, Debug, PartialEq)]
pub struct Word {
    pub answer: String,
    pub english: Vec<String>,
    pub math: u8,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Answer {
    pub single_choice: Vec<String>,
    pub multiple_choice: Vec<String>,
    pub binary_choice: Vec<bool>,
    pub non_choice: Vec<Word>,
}

#[derive(Debug, PartialEq)]
pub enum AnswerState {
    Pending,
    Ready { answer: Answer },
    Error { error: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum AnswerStatus {
    Pending,
    Ready,
    Error,
}

#[derive(Deserialize)]
struct AnswerEnvelope {
    status: AnswerStatus,
    answer: Option<Answer>,
    error: Option<String>,
}

pub type Pages = Vec<u8>;
pub type Missing = HashMap<String, Vec<u8>>;
pub type Formula = Option<Vec<u8>>;

pub struct ApiClient {
    client: Client<EspHttpConnection>,
}

fn check_status(endpoint: &str, status: u16) -> Result<()> {
    if !(200..300).contains(&status) {
        bail!("HTTP endpoint /{endpoint} returned status {status}")
    }
    Ok(())
}

fn read_bounded<E>(
    endpoint: &str,
    mut read: impl FnMut(&mut [u8]) -> std::result::Result<usize, E>,
) -> Result<Vec<u8>>
where
    E: Display,
{
    let mut result = Vec::new();
    let mut buf = [0_u8; RESPONSE_CHUNK_SIZE];
    loop {
        let count = read(&mut buf)
            .map_err(|error| anyhow!("Failed reading HTTP endpoint /{endpoint}: {error}"))?;
        if count == 0 {
            return Ok(result);
        }
        if count > buf.len() {
            bail!("HTTP endpoint /{endpoint} returned an invalid read length {count}")
        }
        if result.len() > MAX_RESPONSE_SIZE.saturating_sub(count) {
            bail!("HTTP endpoint /{endpoint} response exceeds {MAX_RESPONSE_SIZE} bytes")
        }
        result.extend_from_slice(&buf[..count]);
    }
}

fn parse_answer(bytes: &[u8]) -> Result<AnswerState> {
    let response: AnswerEnvelope =
        serde_json::from_slice(bytes).context("Invalid JSON from HTTP endpoint /answer")?;
    match response.status {
        AnswerStatus::Pending => Ok(AnswerState::Pending),
        AnswerStatus::Ready => response
            .answer
            .map(|answer| AnswerState::Ready { answer })
            .context("HTTP endpoint /answer returned ready without an answer"),
        AnswerStatus::Error => response
            .error
            .map(|error| AnswerState::Error { error })
            .context("HTTP endpoint /answer returned error without a reason"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    #[test]
    fn reads_response_larger_than_one_chunk() {
        let expected = (0..4097)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let mut offset = 0;
        let actual = read_bounded("answer", |buf| {
            let count = (expected.len() - offset).min(buf.len());
            buf[..count].copy_from_slice(&expected[offset..offset + count]);
            offset += count;
            Ok::<usize, Infallible>(count)
        })
        .unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn rejects_oversized_response() {
        let mut remaining = MAX_RESPONSE_SIZE + 1;
        let error = read_bounded("answer", |buf| {
            let count = remaining.min(buf.len());
            remaining -= count;
            Ok::<usize, Infallible>(count)
        })
        .unwrap_err();

        assert!(error.to_string().contains("/answer"));
        assert!(error.to_string().contains(&MAX_RESPONSE_SIZE.to_string()));
    }

    #[test]
    fn parses_answer_states_and_string_non_choice_answer() {
        let ready = br#"{
            "status":"ready",
            "answer":{
                "single_choice":["A"],
                "multiple_choice":["AC"],
                "binary_choice":[true],
                "non_choice":[{"answer":"result$", "english":["result"], "math":1}]
            },
            "error":null
        }"#;
        let pending = br#"{"status":"pending","answer":null,"error":null}"#;
        let failed = br#"{"status":"error","answer":null,"error":"worker failed"}"#;

        let AnswerState::Ready { answer } = parse_answer(ready).unwrap() else {
            panic!("expected ready answer")
        };
        assert_eq!(answer.non_choice[0].answer, "result$");
        assert_eq!(parse_answer(pending).unwrap(), AnswerState::Pending);
        assert_eq!(
            parse_answer(failed).unwrap(),
            AnswerState::Error {
                error: "worker failed".to_string()
            }
        );
    }

    #[test]
    fn status_error_names_endpoint_and_status() {
        let error = check_status("pages", 503).unwrap_err().to_string();
        assert!(error.contains("/pages"));
        assert!(error.contains("503"));
    }
}
