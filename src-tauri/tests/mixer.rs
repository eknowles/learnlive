//! Mixer end-to-end with a `file:` source: no models, no audio hardware, so this runs in CI.
//!
//! Regression test for capture ownership. The cpal streams used to be held in the `Mixer`
//! struct returned to the caller, which nothing kept alive — so a live session started, dropped
//! every input stream, and then waited forever on silence. Unit tests never saw it because the
//! offline runner bypasses `Mixer` entirely.

use std::time::{Duration, Instant};

use learnlive_lib::audio::{write_wav16, Mixer, MixerCommand};
use learnlive_lib::types::{MixerSource, SourceRole};

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

fn drain(mixer: &Mixer, for_: Duration) -> (usize, usize, f32) {
    let (mut frames, mut samples, mut peak) = (0usize, 0usize, 0.0f32);
    let start = Instant::now();
    while start.elapsed() < for_ {
        if let Ok(f) = mixer.frames.recv_timeout(Duration::from_millis(200)) {
            frames += 1;
            samples += f.mix.len();
            peak = peak.max(f.mix.iter().fold(0.0f32, |a, b| a.max(b.abs())));
        }
    }
    (frames, samples, peak)
}

#[test]
fn capture_survives_start_returning_and_audio_flows() {
    let path = tone_wav("learnlive-mixer-test.wav", 2, 48_000, 2);
    let src = MixerSource {
        device_id: format!("file:{}", path.display()),
        role: SourceRole::Remote,
        gain: 1.0,
        muted: false,
    };
    let mixer = Mixer::start(&[src]).expect("mixer starts");

    let (frames, samples, peak) = drain(&mixer, Duration::from_secs(3));
    let _ = mixer.commands.send(MixerCommand::Stop);

    // 2 s at 48 kHz stereo resampled to 16 kHz mono, in 20 ms frames.
    assert!(frames > 80, "mixer delivered {frames} frames; capture is not reaching the consumer");
    let secs = samples as f32 / 16_000.0;
    assert!((secs - 2.0).abs() < 0.2, "expected ~2 s of 16 kHz audio, got {secs:.2} s");
    assert!(peak > 0.3, "audio arrived silent (peak {peak:.3}); check gain/downmix");
}

#[test]
fn muting_a_source_silences_the_mix() {
    let path = tone_wav("learnlive-mixer-mute.wav", 2, 48_000, 1);
    let id = format!("file:{}", path.display());
    let src = MixerSource { device_id: id.clone(), role: SourceRole::Remote, gain: 1.0, muted: true };
    let mixer = Mixer::start(&[src]).expect("mixer starts");

    let (frames, _, peak) = drain(&mixer, Duration::from_secs(2));
    let _ = mixer.commands.send(MixerCommand::Stop);

    assert!(frames > 40, "mixer should still emit frames while muted, got {frames}");
    assert!(peak < 1e-6, "muted source still audible (peak {peak})");
}
