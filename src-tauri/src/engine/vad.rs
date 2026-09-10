//! Silero VAD via sherpa-onnx: turns the 20 ms frame stream into utterances.
//!
//! sherpa's `SileroVad` already does the segmentation, so we drive its queue rather than
//! re-implementing silence tracking on top of `is_speech()`. Segments must be `pop`ped or the
//! internal buffer grows without bound.

use std::collections::VecDeque;
use std::path::Path;

use anyhow::{anyhow, Result};
use sherpa_rs::silero_vad::{SileroVad, SileroVadConfig};

use crate::audio::{FRAME_SAMPLES, PIPELINE_RATE};

pub struct Utterance {
    pub pcm: Vec<f32>,
    pub started_ms: u64,
    pub ended_ms: u64,
}

/// Tunables for how speech is cut into utterances. These dominate perceived latency: nothing
/// reaches the screen until an utterance *closes*, so `min_silence_ms` is the floor on how long
/// after you stop talking the first word can appear, and `max_speech_ms` is the worst case if you
/// never pause. Exposed so `bench-latency` can sweep them and so they can become user settings.
#[derive(Debug, Clone)]
pub struct SegmenterOptions {
    /// Silence needed to close an utterance.
    pub min_silence_ms: u64,
    /// Ignore blips shorter than this.
    pub min_speech_ms: u64,
    /// Hard cap so an unbroken monologue still gets chunked.
    pub max_speech_ms: u64,
    /// Silero speech probability threshold.
    pub threshold: f32,
    /// Audio kept from *before* the VAD decided speech had started. Silero needs a few frames of
    /// evidence before it fires, so without this the leading consonant — often the whole first
    /// word — is missing from the audio handed to ASR ("Good morning" arrives as "morning").
    pub preroll_ms: u64,
}

impl Default for SegmenterOptions {
    fn default() -> Self {
        Self { min_silence_ms: 200, min_speech_ms: 250, max_speech_ms: 2_500, threshold: 0.5, preroll_ms: 300 }
    }
}

pub struct Segmenter {
    vad: SileroVad,
    ready: VecDeque<Utterance>,
    /// Rolling copy of recent audio, so an utterance can be extended backwards past the point
    /// where the VAD became confident. It must span an entire utterance plus the closing silence,
    /// because sherpa only hands a segment over once that silence has elapsed — by which point
    /// the segment's own start is already well in the past.
    history: VecDeque<f32>,
    history_cap: usize,
    /// Total samples fed, giving `history` an absolute position on the VAD's sample timeline.
    fed: u64,
    preroll: usize,
}

impl Segmenter {
    pub fn new(model_dir: &Path) -> Result<Self> {
        Self::with_options(model_dir, SegmenterOptions::default())
    }

    pub fn with_options(model_dir: &Path, opts: SegmenterOptions) -> Result<Self> {
        let cfg = SileroVadConfig {
            model: model_dir.join("silero-vad").join("silero_vad.onnx").to_string_lossy().into(),
            window_size: 512,
            sample_rate: PIPELINE_RATE,
            threshold: opts.threshold,
            min_silence_duration: opts.min_silence_ms as f32 / 1000.0,
            min_speech_duration: opts.min_speech_ms as f32 / 1000.0,
            max_speech_duration: opts.max_speech_ms as f32 / 1000.0,
            ..Default::default()
        };
        let ms_to_samples = |ms: u64| (ms * PIPELINE_RATE as u64 / 1000) as usize;
        let preroll = ms_to_samples(opts.preroll_ms);
        // One full utterance + the silence that closes it + the lead-in, with slack.
        let history_cap = ms_to_samples(opts.max_speech_ms + opts.min_silence_ms + opts.preroll_ms + 500);
        Ok(Self {
            vad: SileroVad::new(cfg, 30.0).map_err(|e| anyhow!("silero vad: {e}"))?,
            ready: VecDeque::new(),
            history: VecDeque::with_capacity(history_cap + FRAME_SAMPLES),
            history_cap,
            fed: 0,
            preroll,
        })
    }

    /// Feed one 20 ms frame. Returns (is speech active, a finished utterance if one closed).
    pub fn push(&mut self, frame: &[f32]) -> Result<(bool, Option<Utterance>)> {
        self.vad.accept_waveform(frame.to_vec());
        self.history.extend(frame.iter().copied());
        self.fed += frame.len() as u64;
        while self.history.len() > self.history_cap {
            self.history.pop_front();
        }
        self.drain_segments();
        Ok((self.vad.is_speech(), self.ready.pop_front()))
    }

    fn drain_segments(&mut self) {
        while !self.vad.is_empty() {
            let seg = self.vad.front();
            self.vad.pop();
            let start = seg.start.max(0) as u64;
            let (lead, lead_start) = self.lead_in(start);
            let mut pcm = lead;
            pcm.extend_from_slice(&seg.samples);
            let started_ms = samples_to_ms(lead_start);
            let ended_ms = samples_to_ms(start + seg.samples.len() as u64);
            self.ready.push_back(Utterance { pcm, started_ms, ended_ms });
        }
    }

    /// Audio immediately before `start`, as much as `history` still holds.
    /// Returns (samples, absolute sample index they begin at).
    fn lead_in(&self, start: u64) -> (Vec<f32>, u64) {
        let history_begins = self.fed.saturating_sub(self.history.len() as u64);
        if self.preroll == 0 || start <= history_begins {
            return (Vec::new(), start);
        }
        let want = self.preroll.min((start - history_begins) as usize);
        let from = (start - history_begins) as usize - want;
        let lead: Vec<f32> = self.history.iter().skip(from).take(want).copied().collect();
        (lead, start - want as u64)
    }

    /// Anything the VAD is still holding at the end of a stream.
    pub fn flush(&mut self) -> Vec<Utterance> {
        self.vad.flush();
        self.drain_segments();
        self.ready.drain(..).collect()
    }
}

fn samples_to_ms(samples: u64) -> u64 {
    samples * 1000 / PIPELINE_RATE as u64
}
