use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("tauri: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("unknown site: {0}")]
    UnknownSite(String),
    #[error("login window already open for site {0}")]
    LoginInProgress(String),
    #[error("no cookies stored for site {0}")]
    NoCookies(String),
    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
