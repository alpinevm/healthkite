use crate::http::{HttpError, HttpTransport};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HealthKiteError {
    #[error("HEALTHKITE_TOKEN is missing or wrong")]
    Unauthorized,
    #[error("{0} was not found")]
    NotFound(String),
    #[error("Invalid day snapshot date: {0}. Expected YYYY-MM-DD.")]
    BadDate(String),
    #[error(
        "Cannot reach HealthKite MCP at {0}. Is the iOS app foregrounded and the LAN server enabled?"
    )]
    Unreachable(String),
    #[error("{message}")]
    ServerError { message: String, status: u16 },
}

impl HealthKiteError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "Unauthorized",
            Self::NotFound(_) => "NotFound",
            Self::BadDate(_) => "BadDate",
            Self::Unreachable(_) => "Unreachable",
            Self::ServerError { .. } => "ServerError",
        }
    }
}

#[derive(Clone)]
pub struct HealthKiteClient {
    base_url: String,
    transport: Arc<dyn HttpTransport>,
}

impl HealthKiteClient {
    pub fn new(base_url: impl Into<String>, transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            base_url: base_url.into(),
            transport,
        }
    }

    pub fn status(&self) -> Result<String, HealthKiteError> {
        self.request_text("/", "status")
    }

    pub fn list_workouts(&self, limit: i64, offset: i64) -> Result<String, HealthKiteError> {
        self.request_text(
            &format!("/workouts?limit={limit}&offset={offset}"),
            "workouts list",
        )
    }

    pub fn get_workout(&self, uuid: &str) -> Result<String, HealthKiteError> {
        self.request_text(
            &format!("/workouts/{}", urlencoding::encode(uuid)),
            &format!("workout {uuid}"),
        )
    }

    pub fn list_quantity_types(&self) -> Result<String, HealthKiteError> {
        self.request_text("/quantity-types", "quantity types")
    }

    pub fn get_quantity_series(
        &self,
        quantity_type: &str,
        from: Option<&str>,
        to: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<String, HealthKiteError> {
        let mut query = QueryBuilder::new();
        query.add_optional("from", from);
        query.add_optional("to", to);
        query.add("limit", &limit.to_string());
        query.add("offset", &offset.to_string());
        self.request_text(
            &format!(
                "/quantity/{}{}",
                urlencoding::encode(quantity_type),
                query.finish()
            ),
            &format!("quantity series {quantity_type}"),
        )
    }

    pub fn list_sleep_sessions(
        &self,
        from: Option<&str>,
        to: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<String, HealthKiteError> {
        let mut query = QueryBuilder::new();
        query.add_optional("from", from);
        query.add_optional("to", to);
        query.add("limit", &limit.to_string());
        query.add("offset", &offset.to_string());
        self.request_text(&format!("/sleep{}", query.finish()), "sleep sessions")
    }

    pub fn get_day_snapshot(&self, date: &str) -> Result<String, HealthKiteError> {
        if !is_day_snapshot_date(date) {
            return Err(HealthKiteError::BadDate(date.to_string()));
        }
        self.request_text(
            &format!("/day-snapshot/{}", urlencoding::encode(date)),
            &format!("day snapshot {date}"),
        )
    }

    fn request_text(&self, path: &str, context: &str) -> Result<String, HealthKiteError> {
        let response = self.transport.get(path).map_err(|error| match error {
            HttpError::Connect(message)
            | HttpError::TlsHandshake(message)
            | HttpError::Read(message)
            | HttpError::Write(message) => {
                HealthKiteError::Unreachable(format!("{} ({message})", self.base_url))
            }
            other => HealthKiteError::ServerError {
                message: other.to_string(),
                status: 0,
            },
        })?;

        if (200..300).contains(&response.status) {
            return Ok(response.body);
        }

        match response.status {
            401 => Err(HealthKiteError::Unauthorized),
            404 => Err(HealthKiteError::NotFound(context.to_string())),
            400 if response.body.contains("bad_date") => {
                Err(HealthKiteError::BadDate(context.to_string()))
            }
            status => Err(HealthKiteError::ServerError {
                message: if response.body.is_empty() {
                    response.reason
                } else {
                    response.body
                },
                status,
            }),
        }
    }
}

struct QueryBuilder {
    pairs: Vec<(String, String)>,
}

impl QueryBuilder {
    fn new() -> Self {
        Self { pairs: Vec::new() }
    }

    fn add(&mut self, key: &str, value: &str) {
        self.pairs.push((key.to_string(), value.to_string()));
    }

    fn add_optional(&mut self, key: &str, value: Option<&str>) {
        if let Some(value) = value {
            self.add(key, value);
        }
    }

    fn finish(self) -> String {
        if self.pairs.is_empty() {
            return String::new();
        }
        let encoded = self
            .pairs
            .into_iter()
            .map(|(key, value)| {
                format!(
                    "{}={}",
                    urlencoding::encode(&key),
                    urlencoding::encode(&value)
                )
            })
            .collect::<Vec<_>>()
            .join("&");
        format!("?{encoded}")
    }
}

pub fn is_day_snapshot_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[4] == b'-'
        && bytes[5].is_ascii_digit()
        && bytes[6].is_ascii_digit()
        && bytes[7] == b'-'
        && bytes[8].is_ascii_digit()
        && bytes[9].is_ascii_digit()
}

pub fn integer_arg(value: Option<&Value>, default_value: i64, min: i64, max: Option<i64>) -> i64 {
    let Some(value) = value.and_then(Value::as_i64) else {
        return default_value;
    };
    let upper_bounded = max.map_or(value, |max| value.min(max));
    upper_bounded.max(min)
}

pub fn string_arg(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn optional_string_arg(value: Option<&Value>) -> Option<String> {
    string_arg(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{HttpError, HttpResponse, HttpTransport};
    use std::sync::Arc;

    #[test]
    fn day_snapshot_date_validation_matches_typescript_contract() {
        assert!(is_day_snapshot_date("2026-05-11"));
        assert!(!is_day_snapshot_date("May 11"));
        assert!(!is_day_snapshot_date("2026-5-11"));
    }

    #[test]
    fn integer_arg_defaults_and_clamps() {
        assert_eq!(integer_arg(None, 50, 1, Some(200)), 50);
        assert_eq!(integer_arg(Some(&Value::from(0)), 50, 1, Some(200)), 1);
        assert_eq!(integer_arg(Some(&Value::from(500)), 50, 1, Some(200)), 200);
        assert_eq!(integer_arg(Some(&Value::from(12)), 50, 1, Some(200)), 12);
        assert_eq!(integer_arg(Some(&Value::from(1.5)), 50, 1, Some(200)), 50);
    }

    struct ConstantTransport {
        response: HttpResponse,
    }

    impl HttpTransport for ConstantTransport {
        fn get(&self, _path_and_query: &str) -> Result<HttpResponse, HttpError> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn maps_status_errors() {
        let client = HealthKiteClient::new(
            "https://healthkite.local:5606",
            Arc::new(ConstantTransport {
                response: HttpResponse {
                    status: 401,
                    reason: "Unauthorized".to_string(),
                    body: r#"{"error":"unauthorized"}"#.to_string(),
                },
            }),
        );
        let error = client.status().unwrap_err();
        assert_eq!(error.code(), "Unauthorized");
    }
}
