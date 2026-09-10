//! Whisper (multilingual) through sherpa-onnx. Gives text + detected language per utterance.

use std::path::Path;

use anyhow::{anyhow, Result};
use log::warn;
use parking_lot::Mutex;
use sherpa_rs::whisper::{WhisperConfig, WhisperRecognizer};

use super::{languages, Transcriber};

pub struct WhisperOnnx {
    // sherpa's recognizer is not Sync; serialize calls. ASR is the bottleneck anyway.
    inner: Mutex<WhisperRecognizer>,
    /// The language decoding is pinned to, or empty when auto-detecting.
    pinned: String,
}

impl WhisperOnnx {
    /// `source_lang` is "auto" or an ISO-639-1 code.
    ///
    /// Note the validation: sherpa-onnx looks the language up in the model's `all_language_codes`
    /// metadata and calls `exit(-1)` on a miss — a hard process abort no Rust error handling can
    /// intercept (`offline-whisper-greedy-search-decoder.cc:39`). An empty string is the documented
    /// way to ask for auto-detection; "auto" is not a language code and would kill the app.
    pub fn load(model_dir: &Path, size: &str, source_lang: &str) -> Result<Self> {
        let dir = model_dir.join(crate::models::asr(size).dir);
        let stem = |s: &str| dir.join(format!("{size}-{s}.int8.onnx")).to_string_lossy().to_string();

        let pinned = pin_language(source_lang);

        let cfg = WhisperConfig {
            encoder: stem("encoder"),
            decoder: stem("decoder"),
            tokens: dir.join(format!("{size}-tokens.txt")).to_string_lossy().into(),
            language: pinned.clone(),
            num_threads: Some(4),
            ..Default::default()
        };
        let inner = WhisperRecognizer::new(cfg).map_err(|e| anyhow!("whisper: {e}"))?;
        Ok(Self { inner: Mutex::new(inner), pinned })
    }
}

/// Map a `SessionConfig::source_lang` onto what sherpa-onnx will accept.
///
/// Anything non-empty that is not in the model's language table aborts the process, so this must
/// never let an unrecognised code through — "" (auto-detect) is the only safe fallback.
fn pin_language(source_lang: &str) -> String {
    match source_lang {
        "auto" | "" => String::new(),
        code if languages::by_code(code).is_some() => code.to_string(),
        other => {
            warn!("unknown source language {other:?}; falling back to auto-detection");
            String::new()
        }
    }
}

impl Transcriber for WhisperOnnx {
    fn transcribe(&self, pcm16k: &[f32], _lang_hint: &str) -> Result<(String, String)> {
        let res = self.inner.lock().transcribe(16_000, pcm16k);

        // Whisper detects 99 languages; we can only translate the 16 in `languages.rs`. Reporting
        // one we can't handle would fail the whole sentence in the translator, so fall back to
        // whatever the session pinned (else English) and keep the transcript.
        let lang = if !self.pinned.is_empty() {
            self.pinned.clone()
        } else if languages::by_code(&res.lang).is_some() {
            res.lang.clone()
        } else {
            if !res.lang.is_empty() {
                warn!("detected unsupported language {:?}; treating as English", res.lang);
            }
            "en".to_string()
        };
        Ok((lang, res.text.trim().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// sherpa-onnx `exit(-1)`s on a language it does not know, taking the whole app with it, so
    /// the only values we may ever hand it are "" or a code from the model's table.
    #[test]
    fn only_empty_or_known_codes_reach_sherpa() {
        assert_eq!(pin_language("auto"), "", "\"auto\" is not a language code; it must become empty");
        assert_eq!(pin_language(""), "");
        assert_eq!(pin_language("ru"), "ru");
        assert_eq!(pin_language("en"), "en");
        for bogus in ["klingon", "zz", "en-GB", "AUTO", "ru_RU"] {
            assert_eq!(pin_language(bogus), "", "{bogus} must fall back to auto-detection");
        }
        for l in languages::LANGUAGES {
            assert_eq!(pin_language(l.code), l.code, "{} should be passed through", l.code);
        }
    }
}
