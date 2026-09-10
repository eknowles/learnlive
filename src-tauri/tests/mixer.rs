//! Mixer end-to-end with a `file:` source: no models, no audio hardware, so this runs in CI.
//!
//! Regression test for capture ownership. The cpal streams used to be held in the `Mixer` struct
//! returned to the caller, which nothing kept alive — so a live session started, dropped every
//! input stream, and then waited forever on silence. Unit tests never saw it because the offline
//! runner bypasses `Mixer` entirely.
//!
//! These wait for a quantity of audio rather than sampling a fixed wall-clock window: the source
//! is paced in real time, and a loaded CI runner delivers it slower than a quiet laptop. Asserting
//! on *throughput* made this flaky; asserting on *content, eventually* is what we actually mean.

use std::time::{Duration, Instant};

use learnlive_lib::audio::{write_wav16, Mixer, MixerCommand, FRAME_SAMPLES, PIPELINE_RATE};
use learnlive_lib::types::{MixerSource, SourceRole};

/// Generous: only ever hit if audio is not flowing at all, which is the bug under test.
const DEADLINE: Duration = Duration::from_secs(30);

fn tone_wav(name: &str, secs: u32, rate: u32, channels: u16) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(name);
    let n = rate * secs;
    let s: Vec<f32> = (0..n)
        .flat_map(|i| {
            let v = (i as f32 / rate as f32 * 440.0 * std::f32::consts::TAU).sin() * 0.4;
            std::iter::repeat(v).take(channels as usize)
        })
        .collect();
    write_wav16(&path, rate, channels, &s).expect("write fixture");
    path
}

fn source(path: &std::path::Path, muted: bool) -> MixerSource {
    MixerSource { device_id: format!("file:{}", path.display()), role: SourceRole::Remote, gain: 1.0, muted }
}

struct Drained {
    frames: usize,
    samples: usize,
    peak: f32,
}

/// Collect until `want` samples of mixed audio have arrived, or we give up.
fn drain(mixer: &Mixer, want: usize) -> Drained {
    let start = Instant::now();
    let mut d = Drained { frames: 0, samples: 0, peak: 0.0 };
    while d.samples < want && start.elapsed() < DEADLINE {
        if let Ok(f) = mixer.frames.recv_timeout(Duration::from_millis(500)) {
            d.frames += 1;
            d.samples += f.mix.len();
            d.peak = d.peak.max(f.mix.iter().fold(0.0f32, |a, b| a.max(b.abs())));
        }
    }
    d
}

#[test]
fn capture_survives_start_returning_and_audio_flows() {
    let path = tone_wav("learnlive-mixer-test.wav", 2, 48_000, 2);
    let mixer = Mixer::start(&[source(&path, false)]).expect("mixer starts");

    // 2 s at 48 kHz stereo, resampled to 16 kHz mono. Allow for the resampler holding a partial
    // block back rather than demanding every last sample.
    let want = PIPELINE_RATE as usize * 2 - FRAME_SAMPLES * 4;
    let d = drain(&mixer, want);
    let _ = mixer.commands.send(MixerCommand::Stop);

    assert!(
        d.samples >= want,
        "only {} of {want} samples arrived ({} frames); capture is not reaching the consumer",
        d.samples,
        d.frames
    );
    assert!(d.peak > 0.3, "audio arrived silent (peak {:.3}); check gain/downmix", d.peak);
}

#[test]
fn muting_a_source_silences_the_mix() {
    let path = tone_wav("learnlive-mixer-mute.wav", 2, 48_000, 1);
    let mixer = Mixer::start(&[source(&path, true)]).expect("mixer starts");

    // A muted source must still be clocked through the mixer — the frames keep coming, silent.
    let want = PIPELINE_RATE as usize; // 1 s is plenty to prove it
    let d = drain(&mixer, want);
    let _ = mixer.commands.send(MixerCommand::Stop);

    assert!(d.samples >= want, "muted source stopped producing frames ({} samples)", d.samples);
    assert!(d.peak < 1e-6, "muted source still audible (peak {})", d.peak);
}
