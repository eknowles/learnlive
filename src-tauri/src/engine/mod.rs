//! ML engines. Each is behind a trait so a model can be swapped without touching the pipeline.
//! Engines are lazily loaded on first session start (after `prepare_models` has fetched files).

pub mod asr;
pub mod diarize;
pub mod grammar;
pub mod languages;
pub mod translate;
pub mod tts;
pub mod vad;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;

use crate::types::{SessionConfig, Token};

/// `ort`'s builder errors are `ort::Error<SessionBuilder>` — they hand the builder back so you
/// can retry, which makes them neither `Send` nor `Sync` and so unusable with `anyhow`.
/// Stringify at the boundary; nothing upstream inspects them.
pub(crate) fn ort_err<E: std::fmt::Display>(e: E) -> anyhow::Error {
    anyhow::anyhow!("onnxruntime: {e}")
}

pub trait Transcriber: Send + Sync {
    /// Returns (language code, text). `lang_hint` = "auto" or ISO-639-1.
    fn transcribe(&self, pcm16k: &[f32], lang_hint: &str) -> Result<(String, String)>;
}

pub trait SpeakerEmbedder: Send + Sync {
    fn embed(&self, pcm16k: &[f32]) -> Result<Vec<f32>>;
}

pub trait Translator: Send + Sync {
    fn translate(&self, text: &str, from: &str, to: &str) -> Result<String>;
}

pub trait GrammarAnalyzer: Send + Sync {
    fn analyze(&self, text: &str, lang: &str) -> Result<Vec<Token>>;
}

pub trait Synthesizer: Send + Sync {
    /// Returns (mono samples, sample rate).
    fn synthesize(&self, text: &str) -> Result<(Vec<f32>, u32)>;
}

/// Everything one session needs, already loaded. Cheap to clone (all Arcs).
#[derive(Clone)]
pub struct Loaded {
    pub asr: Arc<dyn Transcriber>,
    pub translator: Arc<dyn Translator>,
    pub grammar: Option<Arc<dyn GrammarAnalyzer>>,
    pub speaker: Option<Arc<dyn SpeakerEmbedder>>,
    pub tts: std::collections::HashMap<String, Arc<dyn Synthesizer>>,
}

/// Lazily-initialised engine bundle shared across sessions.
pub struct Engines {
    pub model_dir: PathBuf,
    pub asr: Mutex<Option<Arc<dyn Transcriber>>>,
    pub speaker: Mutex<Option<Arc<dyn SpeakerEmbedder>>>,
    pub translator: Mutex<Option<Arc<dyn Translator>>>,
    pub grammar: Mutex<Option<Arc<dyn GrammarAnalyzer>>>,
    /// keyed by language
    pub tts: Mutex<std::collections::HashMap<String, Arc<dyn Synthesizer>>>,
    loaded_asr: Mutex<String>,
}

impl Engines {
    pub fn new(model_dir: PathBuf) -> Self {
        Self {
            model_dir,
            asr: Mutex::new(None),
            speaker: Mutex::new(None),
            translator: Mutex::new(None),
            grammar: Mutex::new(None),
            tts: Mutex::new(Default::default()),
            loaded_asr: Mutex::new(String::new()),
        }
    }

    /// Load everything a session needs and hand back a snapshot. Cheap if already loaded.
    pub fn load(&self, cfg: &SessionConfig) -> Result<Loaded> {
        self.warm(cfg)?;
        Ok(Loaded {
            asr: self.asr.lock().clone().ok_or_else(|| anyhow::anyhow!("ASR not loaded"))?,
            translator: self.translator.lock().clone().ok_or_else(|| anyhow::anyhow!("translator not loaded"))?,
            grammar: self.grammar.lock().clone(),
            speaker: if cfg.diarize { self.speaker.lock().clone() } else { None },
            tts: self.tts.lock().clone(),
        })
    }

    /// Ensure models are in memory. Prefer `load()`.
    pub fn warm(&self, cfg: &SessionConfig) -> Result<()> {
        // The source language is baked into the recognizer at construction, so it is part of the
        // cache key alongside the model size.
        let asr_key = format!("{}|{}", cfg.asr_model, cfg.source_lang);
        if self.asr.lock().is_none() || *self.loaded_asr.lock() != asr_key {
            let engine = asr::WhisperOnnx::load(&self.model_dir, &cfg.asr_model, &cfg.source_lang)?;
            *self.asr.lock() = Some(Arc::new(engine));
            *self.loaded_asr.lock() = asr_key;
        }
        if cfg.diarize && self.speaker.lock().is_none() {
            *self.speaker.lock() = Some(Arc::new(diarize::Eres2Net::load(&self.model_dir)?));
        }
        if self.translator.lock().is_none() {
            *self.translator.lock() = Some(Arc::new(translate::Nllb::load(&self.model_dir)?));
        }
        if self.grammar.lock().is_none() {
            // Optional: without it, segments simply carry no tokens and the UI shows plain text.
            match grammar::UdPos::load(&self.model_dir) {
                Ok(g) => *self.grammar.lock() = Some(Arc::new(g)),
                Err(e) => log::warn!("grammar tagger unavailable, word cards disabled: {e:#}"),
            }
        }
        if cfg.speak_translations {
            let mut tts = self.tts.lock();
            for lang in [&cfg.learning, &cfg.native] {
                if !tts.contains_key(lang) {
                    if let Some(v) = tts::Piper::load(&self.model_dir, lang)? {
                        tts.insert(lang.clone(), Arc::new(v));
                    }
                }
            }
        }
        Ok(())
    }
}
