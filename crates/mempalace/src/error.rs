use thiserror::Error;

#[derive(Debug, Error)]
pub enum MpError {
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Embedding error: {0}")]
    Embedding(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Palace not found: {0}")]
    PalaceNotFound(String),
    #[error("Mine already running")]
    MineAlreadyRunning,
}
