#[derive(Debug, thiserror::Error)]
pub enum RazeTradingError {
    #[error("connection error: {0}")]
    Connection(String),
    #[error("auth error: {0}")]
    Auth(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("rate limit exceeded{}", .retry_after.map(|s| format!(" (retry after {s}s)")).unwrap_or_default())]
    RateLimit {
        /// `Retry-After` header value in seconds, if the server sent one.
        retry_after: Option<u64>,
        message: String,
    },
    #[error("server error ({status}): {message}")]
    Server { status: u16, message: String },
    #[error("decode error: {0}")]
    Decode(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for RazeTradingError {
    fn from(err: reqwest::Error) -> Self {
        RazeTradingError::Connection(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, RazeTradingError>;
