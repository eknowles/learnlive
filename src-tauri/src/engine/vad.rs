//! Silero VAD via sherpa-onnx: turns the 20 ms frame stream into utterances.

use std::path::Path;

use anyhow::Result;
use sherpa_rs::vad::{Vad, VadConfig};

pub struct Utterance {
    pub pcm: Vec<f32>,
    pub started_ms: u64,
    pub ended_ms: u64,
}

pub struct Segmenter {
    vad: Vad,
    clock_ms: u64,
    /// Silence needed to close an utterance. Lower = snappier, higher = fewer mid-sentence splits.
    min_silence_ms: u64,
    /// Hard cap so long monologues still get translated in chunks.
    max_utterance_ms: u64,
    pending: Vec<f32>,
    pending_start: Option<u64>,
    silence_run_ms: u64,
}

impl Segmenter {
    pub fn new(model_dir: &Path) -> Result<Self> {
        let cfg = VadConfig {
            model: model_dir.join("silero-vad").join("silero_vad.onnx").to_string_lossy().into(),
            window_size: 320,
            sample_rate: 16_000,
            threshold: 0.5,
            min_silence_duration: 0.3,
            min_speech_duration: 0.25,
            ..Default::default()
        };
        Ok(Self {
            vad: Vad::new(cfg, 30.0)?,
            clock_ms: 0,
            min_silence_ms: 700,
            max_utterance_ms: 12_000,
            pending: vec![],
            pending_start: None,
            silence_run_ms: 0,
        })
    }

    /// Feed one 20 ms frame. Returns a finished utterance when a pause (or the cap) is hit.
    pub fn push(&mut self, frame: &[f32]) -> Result<(bool, Option<Utterance>)> {
        let frame_ms = (frame.len() as u64 * 1000) / 16_000;
        self.vad.accept_waveform(frame.to_vec());
        let speaking = self.vad.is_speech();
        self.clock_ms += frame_ms;

        if speaking {
            if self.pending_start.is_none() {
                self.pending_start = Some(self.clock_ms - frame_ms);
            }
            self.silence_run_ms = 0;
            self.pending.extend_from_slice(frame);
        } else if self.pending_start.is_some() {
            self.silence_run_ms += frame_ms;
            self.pending.extend_from_slice(frame); // keep a little tail for natural cut-offs
        }

        let dur = self.pending_start.map(|s| self.clock_ms - s).unwrap_or(0);
        let should_close = self.pending_start.is_some()
            && (self.silence_run_ms >= self.min_silence_ms || dur >= self.max_utterance_ms);

        if should_close {
            let started_ms = self.pending_start.take().unwrap();
            let pcm = std::mem::take(&mut self.pending);
            self.silence_run_ms = 0;
            return Ok((speaking, Some(Utterance { pcm, started_ms, ended_ms: self.clock_ms })));
        }
        Ok((speaking, None))
    }
}
