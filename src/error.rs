use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Bitcoin CLI error: {0}")]
    BitcoinCli(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("API request error: {0}")]
    ApiRequest(#[from] ureq::Error),

    #[error("Address parsing error: {0}")]
    AddressParse(String),

    #[error("Clipboard error: {0}")]
    Clipboard(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type AppResult<T> = Result<T, AppError>;
