//! Multi-source capture and mixing.
//!
//! Each `MixerSource` gets its own cpal input stream. Frames are converted to mono f32,
//! resampled to 16 kHz, scaled by gain, then summed into a single mix that the pipeline
//! consumes. Per-source streams are also kept separately so diarization can be
//! short-circuited (local mic = "you") and so meters can show each input.

pub mod capture;
pub mod mixer;
pub mod resample;
pub mod playback;

pub use capture::{list_devices, Capture};
pub use mixer::{Mixer, MixerCommand, MixFrame};
pub use playback::Player;

/// Everything downstream of the mixer runs at this rate.
pub const PIPELINE_RATE: u32 = 16_000;
/// Mixer emits frames of this many samples (20 ms at 16 kHz), matching Silero VAD's window.
pub const FRAME_SAMPLES: usize = 320;
