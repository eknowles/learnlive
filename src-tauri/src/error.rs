//! One error type crossing the Tauri boundary. The UI gets `{ kind, message }` so it can
//! decide between "show a toast" and "open the permissions help".

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Bad input from the UI (no sources, unknown language…)
    Invalid,
    /// OS said no (calendar, microphone)
    Permission,
    /// Model files missing / download failed
    Models,
    /// cpal / device problems
    Audio,
    /// Anything else — see message
    Internal,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppError { pub kind: Kind, pub message: String }

impl AppError {
    pub fn invalid(m: impl Into<String>) -> Self { Self { kind: Kind::Invalid, message: m.into() } }
    pub fn permission(m: impl Into<String>) -> Self { Self { kind: Kind::Permission, message: m.into() } }
    pub fn models(m: impl Into<String>) -> Self { Self { kind: Kind::Models, message: m.into() } }
    pub fn audio(m: impl Into<String>) -> Self { Self { kind: Kind::Audio, message: m.into() } }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        let message = format!("{e:#}");
        let l = message.to_lowercase();
        let kind = if l.contains("access") && (l.contains("declined") || l.contains("denied")) { Kind::Permission }
            else if l.contains("device not found") || l.contains("stream") { Kind::Audio }
            else if l.contains("model") || l.contains("onnx") || l.contains("download") { Kind::Models }
            else { Kind::Internal };
        Self { kind, message }
    }
}
impl From<tauri::Error> for AppError { fn from(e: tauri::Error) -> Self { Self { kind: Kind::Internal, message: e.to_string() } } }
impl From<tokio::task::JoinError> for AppError { fn from(e: tokio::task::JoinError) -> Self { Self { kind: Kind::Internal, message: e.to_string() } } }
impl std::fmt::Display for AppError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.message) } }

pub type CmdResult<T> = Result<T, AppError>;
