use std::time::Duration;

use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::error::{RazeTradingError, Result};

/// How a credential is sent on the wire:
///
/// - `sk_…` / `eyJ…` → `Authorization: Bearer …`.
/// - anything else, a license key included → `X-Api-Key`, which is the
///   transport the router checks first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CredentialKind {
    ApiKey,
    Jwt,
    Wrapper,
}

pub(crate) fn classify_credential(cred: &str) -> CredentialKind {
    if cred.starts_with("sk_") {
        CredentialKind::ApiKey
    } else if cred.starts_with("eyJ") {
        CredentialKind::Jwt
    } else {
        CredentialKind::Wrapper
    }
}

pub(crate) struct HttpClient {
    client: Client,
    base_url: String,
    max_retries: u32,
}

impl HttpClient {
    pub fn new(base_url: &str, api_key: &str, timeout: Duration, max_retries: u32) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert("user-agent", "raze-trading-rust/0.3.0".parse().unwrap());
        if !api_key.is_empty() {
            // Route the credential to the header the server expects for its
            // kind, instead of blindly sending X-Api-Key (which silently
            // mis-sends JWTs / non-wrapper sk_ keys).
            let value = api_key
                .parse()
                .map_err(|_| RazeTradingError::Auth("invalid api key header value".into()))?;
            match classify_credential(api_key) {
                CredentialKind::ApiKey | CredentialKind::Jwt => {
                    let bearer: reqwest::header::HeaderValue = format!("Bearer {api_key}")
                        .parse()
                        .map_err(|_| RazeTradingError::Auth("invalid api key header value".into()))?;
                    headers.insert("authorization", bearer);
                }
                CredentialKind::Wrapper => {
                    headers.insert("x-api-key", value);
                }
            }
        }

        let client = Client::builder()
            .default_headers(headers)
            .timeout(timeout)
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
            max_retries,
        })
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        self.request(|| {
            let mut req = self.client.get(&url);
            for (k, v) in params {
                req = req.query(&[(k, v)]);
            }
            req
        })
        .await
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let json = serde_json::to_value(body)?;
        self.request(|| self.client.post(&url).json(&json)).await
    }

    async fn request<T, F>(&self, build: F) -> Result<T>
    where
        T: DeserializeOwned,
        F: Fn() -> reqwest::RequestBuilder,
    {
        let mut last_err: Option<RazeTradingError> = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(200 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
            }

            let res = match build().send().await {
                Ok(r) => r,
                Err(e) => {
                    last_err = Some(RazeTradingError::Connection(e.to_string()));
                    continue;
                }
            };

            let status = res.status();
            if status.is_success() {
                return res
                    .json::<T>()
                    .await
                    .map_err(|e| RazeTradingError::Decode(e.to_string()));
            }

            // Capture Retry-After (seconds) before consuming the body.
            let retry_after = res
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok());

            let err = parse_error(res, retry_after).await;

            match status {
                StatusCode::UNAUTHORIZED => return Err(err),
                StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => return Err(err),
                StatusCode::TOO_MANY_REQUESTS => return Err(err),
                s if s.is_server_error() => {
                    last_err = Some(err);
                    continue;
                }
                _ => return Err(err),
            }
        }

        Err(last_err.unwrap_or_else(|| RazeTradingError::Other("request failed".into())))
    }
}

async fn parse_error(res: Response, retry_after: Option<u64>) -> RazeTradingError {
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    let message = extract_error_message(&body).unwrap_or_else(|| format!("HTTP {status}"));

    match status {
        StatusCode::UNAUTHORIZED => RazeTradingError::Auth(message),
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
            RazeTradingError::Validation(message)
        }
        StatusCode::TOO_MANY_REQUESTS => RazeTradingError::RateLimit {
            retry_after,
            message,
        },
        s if s.is_server_error() => RazeTradingError::Server {
            status: s.as_u16(),
            message,
        },
        _ => RazeTradingError::Other(message),
    }
}

fn extract_error_message(body: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(s) = v.get("error").and_then(|e| e.as_str()) {
            return Some(s.to_owned());
        }
    }
    if !body.is_empty() && body.len() < 500 {
        return Some(body.to_owned());
    }
    None
}
