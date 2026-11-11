use std::path::PathBuf;
use thiserror::Error;

/// HUMBOLDT core error type.
#[derive(Debug, Error)]
pub enum HumboldtError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Port allocation exhausted in range {start}-{end}")]
    PortAllocationExhausted { start: u16, end: u16 },

    #[error("Another instance of HUMBOLDT is already running on port {0}")]
    InstanceAlreadyRunning(u16),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Security vault error: {0}")]
    Security(String),

    #[error("Alert notification error: {0}")]
    Alert(String),

    #[error("Path resolution error for {0}")]
    PathNotFound(PathBuf),
}

pub type Result<T> = std::result::Result<T, HumboldtError>;
