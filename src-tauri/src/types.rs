//! Types shared between the pipeline and the UI (serialised to JSON over Tauri events/commands).

use serde::{Deserialize, Serialize};

/// A capturable audio device as seen by cpal.
#[derive(Debug, Clone, Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub input_channels: u16,
    pub default_sample_rate: u32,
    /// Heuristic: BlackHole / Loopback / "Aggregate" devices are how call audio gets in.
    pub is_loopback: bool,
}

/// One input feeding the mixer. `role` decides what the pipeline does with the speech.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerSource {
    pub device_id: String,
    /// "remote" = other people on the call (via loopback device); "local" = your own mic.
    pub role: SourceRole,
    /// Linear gain 0.0–2.0.
    pub gain: f32,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceRole {
    Remote,
    Local,
}

/// Session configuration sent by the UI when the user presses "Start".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub sources: Vec<MixerSource>,
    /// Language you are learning (target of translation for remote speech).
    pub learning: String,
    /// Language you already speak (target of translation for anything not in `learning`).
    pub native: String,
    /// "auto" or an ISO-639-1 code. Auto lets Whisper detect per utterance.
    pub source_lang: String,
    /// Read translations aloud, ducked under the original audio.
    pub speak_translations: bool,
    /// 0.0–1.0 how much to duck the original while TTS plays.
    pub duck_amount: f32,
    /// Whisper model size: tiny | base | small | medium | large-v3-turbo
    pub asr_model: String,
    pub diarize: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            sources: vec![],
            learning: "ru".into(),
            native: "en".into(),
            source_lang: "auto".into(),
            speak_translations: true,
            duck_amount: 0.5,
            asr_model: "small".into(),
            diarize: true,
        }
    }
}

/// A token in the *translated* sentence with morphological info for the hover card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub text: String,
    pub lemma: String,
    /// Universal Dependencies POS tag: NOUN, VERB, ADJ, ADV, PRON, ADP, CONJ, DET, NUM, PART, PROPN, AUX, INTJ, X
    pub pos: String,
    /// UD morphological features, e.g. {"Case":"Gen","Number":"Plur"}
    pub feats: std::collections::BTreeMap<String, String>,
    /// Byte range of this token in the source sentence it aligns to, if the aligner found one.
    pub aligned_to: Option<(usize, usize)>,
}

/// A finished utterance, emitted to the UI as the `segment` event.
#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub id: String,
    pub speaker: SpeakerRef,
    pub role: SourceRole,
    pub started_ms: u64,
    pub ended_ms: u64,
    pub source_lang: String,
    pub source_text: String,
    pub target_lang: String,
    pub target_text: String,
    pub tokens: Vec<Token>,
    /// Path to the original audio clip (wav) so the UI can replay what was actually said.
    pub clip_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeakerRef {
    pub id: u32,
    /// Auto-assigned "Speaker 1" until the user renames it.
    pub label: String,
    /// Cosine similarity to the speaker centroid — lets the UI show "unsure" states.
    pub confidence: f32,
}

/// Live feedback while an utterance is still being spoken (partial ASR).
#[derive(Debug, Clone, Serialize)]
pub struct Partial {
    pub speaker: Option<u32>,
    pub text: String,
}

/// Progress for model downloads / warm-up.
#[derive(Debug, Clone, Serialize)]
pub struct ModelProgress {
    pub model: String,
    pub bytes: u64,
    pub total: Option<u64>,
    pub done: bool,
}

/// Levels for the mixer meters (emitted ~15×/s).
#[derive(Debug, Clone, Serialize)]
pub struct Levels {
    pub per_source: Vec<(String, f32)>,
    pub mix: f32,
    pub speech_active: bool,
}
