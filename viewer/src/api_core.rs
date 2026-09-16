use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::fmt::Display;

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

#[derive(Debug, PartialEq)]
pub enum NonChoicePart {
    Text(String),
    Formula(u8),
    English(String),
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

pub fn check_status(endpoint: &str, status: u16) -> Result<()> {
    if !(200..300).contains(&status) {
        bail!("HTTP endpoint /{endpoint} returned status {status}")
    }
    Ok(())
}

pub fn read_bounded<E>(
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

pub fn parse_answer(bytes: &[u8]) -> Result<AnswerState> {
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

pub fn formula_api_position(question_index: u8, formula_number: u8) -> (u8, u8) {
    (question_index, formula_number.saturating_sub(1))
}

pub fn parse_non_choice(answer: &str, english: &[String], math: u8) -> Vec<NonChoicePart> {
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut cursor = 0;
    let mut text_start = 0;
    while let Some(offset) = answer[cursor..].find('$') {
        let start = cursor + offset;
        let Some((end_char, is_formula)) =
            answer
                .as_bytes()
                .get(start + 1)
                .and_then(|kind| match kind {
                    b'(' => Some((b')', true)),
                    b'[' => Some((b']', false)),
                    _ => None,
                })
        else {
            cursor = start + 1;
            continue;
        };
        let number_start = start + 2;
        let Some(end_offset) = answer[number_start..]
            .as_bytes()
            .iter()
            .position(|value| *value == end_char)
        else {
            cursor = start + 1;
            continue;
        };
        let end = number_start + end_offset;
        let number_text = &answer[number_start..end];
        if number_text.is_empty() || !number_text.bytes().all(|value| value.is_ascii_digit()) {
            cursor = start + 1;
            continue;
        }

        text.push_str(&answer[text_start..start]);
        let number = number_text.parse::<usize>().ok();
        let part = if is_formula {
            number
                .filter(|number| *number > 0 && *number <= math as usize)
                .map(|number| NonChoicePart::Formula(number as u8))
        } else {
            number
                .and_then(|number| number.checked_sub(1))
                .and_then(|index| english.get(index))
                .cloned()
                .map(NonChoicePart::English)
        };
        if let Some(part) = part {
            if !text.is_empty() {
                parts.push(NonChoicePart::Text(std::mem::take(&mut text)));
            }
            parts.push(part);
        }
        text_start = end + 1;
        cursor = text_start;
    }
    text.push_str(&answer[text_start..]);
    if !text.is_empty() {
        parts.push(NonChoicePart::Text(text));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    #[test]
    fn reads_response_larger_than_one_chunk() {
        let long_answer = "x".repeat(RESPONSE_CHUNK_SIZE + 37);
        let expected = format!(
            r#"{{"status":"ready","answer":{{"single_choice":[],"multiple_choice":[],"binary_choice":[],"non_choice":[{{"answer":"{long_answer}","english":[],"math":0}}]}},"error":null}}"#
        )
        .into_bytes();
        let mut offset = 0;
        let actual = read_bounded("answer", |buf| {
            let count = (expected.len() - offset).min(buf.len());
            buf[..count].copy_from_slice(&expected[offset..offset + count]);
            offset += count;
            Ok::<usize, Infallible>(count)
        })
        .unwrap();

        assert_eq!(actual, expected);
        let AnswerState::Ready { answer } = parse_answer(&actual).unwrap() else {
            panic!("expected ready answer")
        };
        assert_eq!(answer.non_choice[0].answer, long_answer);
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

    #[test]
    fn maps_formula_number_to_zero_based_api_position() {
        assert_eq!(formula_api_position(2, 1), (2, 0));
        assert_eq!(formula_api_position(2, 3), (2, 2));
    }

    #[test]
    fn parses_plain_non_choice_text() {
        assert_eq!(
            parse_non_choice("纯文本", &[], 0),
            vec![NonChoicePart::Text("纯文本".to_string())]
        );
    }

    #[test]
    fn parses_single_formula_placeholder() {
        assert_eq!(
            parse_non_choice("前$(1)后", &[], 1),
            vec![
                NonChoicePart::Text("前".to_string()),
                NonChoicePart::Formula(1),
                NonChoicePart::Text("后".to_string()),
            ]
        );
    }

    #[test]
    fn parses_single_english_placeholder() {
        assert_eq!(
            parse_non_choice("前$[1]后", &["answer".to_string()], 0),
            vec![
                NonChoicePart::Text("前".to_string()),
                NonChoicePart::English("answer".to_string()),
                NonChoicePart::Text("后".to_string()),
            ]
        );
    }

    #[test]
    fn parses_mixed_formula_and_english_placeholders_in_order() {
        assert_eq!(
            parse_non_choice("甲$(2)乙$[1]丙$(1)丁", &["english".to_string()], 2,),
            vec![
                NonChoicePart::Text("甲".to_string()),
                NonChoicePart::Formula(2),
                NonChoicePart::Text("乙".to_string()),
                NonChoicePart::English("english".to_string()),
                NonChoicePart::Text("丙".to_string()),
                NonChoicePart::Formula(1),
                NonChoicePart::Text("丁".to_string()),
            ]
        );
    }

    #[test]
    fn safely_degrades_invalid_and_out_of_range_placeholders() {
        assert_eq!(
            parse_non_choice("前$(2)中$[2]后", &["only".to_string()], 1),
            vec![NonChoicePart::Text("前中后".to_string())]
        );
        assert_eq!(
            parse_non_choice("前$(x)后", &[], 0),
            vec![NonChoicePart::Text("前$(x)后".to_string())]
        );
    }
}
