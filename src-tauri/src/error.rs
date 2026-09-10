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
pub struct AppError {
    pub kind: Kind,
    pub message: String,
}

impl AppError {
    pub fn invalid(m: impl Into<String>) -> Self {
        Self { kind: Kind::Invalid, message: m.into() }
    }
    pub fn permission(m: impl Into<String>) -> Self {
        Self { kind: Kind::Permission, message: m.into() }
    }
    pub fn models(m: impl Into<String>) -> Self {
        Self { kind: Kind::Models, message: m.into() }
    }
    pub fn audio(m: impl Into<String>) -> Self {
        Self { kind: Kind::Audio, message: m.into() }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        let message = format!("{e:#}");
        Self { kind: classify(&message), message }
    }
}

/// Best-effort bucketing of an `anyhow` chain, so the UI can offer the right next step.
///
/// Matches phrases this crate actually produces (see `audio`, `models`, `calendar`) rather than
/// broad words: "download" and "stream" appear throughout onnxruntime's internal errors, and
/// telling someone to "Start again to re-download" because a tensor shape mismatched is worse
/// than saying nothing.
fn classify(message: &str) -> Kind {
    const PERMISSION: &[&str] = &["was declined", "access denied", "not permitted", "permission prompt timed out"];
    const AUDIO: &[&str] = &[
        "audio device not found",
        "no output device",
        "unsupported sample format",
        "default input config",
        "stream error",
        "mixer thread died",
        "audio output thread died",
    ];
    const MODELS: &[&str] = &["model", ".onnx", "onnxruntime", "tokenizer", "get http", "clearing partial download"];

    let l = message.to_lowercase();
    let hit = |set: &[&str]| set.iter().any(|p| l.contains(p));
    if hit(PERMISSION) {
        Kind::Permission
    } else if hit(AUDIO) {
        Kind::Audio
    } else if hit(MODELS) {
        Kind::Models
    } else {
        Kind::Internal
    }
}
impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        Self { kind: Kind::Internal, message: e.to_string() }
    }
}
impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self { kind: Kind::Internal, message: e.to_string() }
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub type CmdResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    fn kind_of(m: &str) -> Kind {
        AppError::from(anyhow::anyhow!("{m}")).kind
    }

    #[test]
    fn classifies_the_messages_this_crate_produces() {
        assert!(matches!(kind_of("audio device not found: BlackHole 2ch"), Kind::Audio));
        assert!(matches!(kind_of("no output device"), Kind::Audio));
        assert!(matches!(kind_of("Calendar access was declined. Allow LearnLive in..."), Kind::Permission));
        assert!(matches!(kind_of("GET https://example.com/model.onnx"), Kind::Models));
        assert!(matches!(kind_of("tokenizer lacks rus_Cyrl"), Kind::Models));
    }

    #[test]
    fn onnxruntime_internals_are_not_sold_as_a_failed_download() {
        // Used to hit the `contains("download")` / `contains("stream")` arms and tell the user
        // to re-download or change audio device.
        assert!(matches!(kind_of("Non-zero status code returned while running Reshape node"), Kind::Internal));
        assert!(matches!(kind_of("invalid utf-8 sequence"), Kind::Internal));
    }
}
