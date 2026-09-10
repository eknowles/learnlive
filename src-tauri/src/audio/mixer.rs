//! Sums N resampled sources into one 16 kHz stream, in fixed 20 ms frames.
//!
//! The cpal input streams are `!Send`, so they are opened *on the mixer thread* and owned by it
//! for its whole life. That keeps `Mixer` itself a pair of channel endpoints (Send + Sync), and
//! it means capture cannot outlive — or, more importantly, die before — the thread consuming it.

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use log::{error, info};

use super::capture::{Capture, RawChunk};
use super::resample::ToPipelineRate;
use super::FRAME_SAMPLES;
use crate::types::{MixerSource, SourceRole};

/// One 20 ms frame of mixed audio plus the per-source contributions.
pub struct MixFrame {
    pub mix: Vec<f32>,
    /// RMS per source id, for meters and for cheap "who is loudest" hints.
    pub rms: Vec<(String, f32)>,
    /// Which role dominated this frame (local mic vs remote call audio).
    pub dominant: Option<SourceRole>,
}

pub enum MixerCommand {
    SetGain {
        device_id: String,
        gain: f32,
    },
    SetMuted {
        device_id: String,
        muted: bool,
    },
    /// Multiply all *remote* sources by this factor (used for ducking while TTS plays).
    Duck(f32),
    Stop,
}

struct Live {
    gain: f32,
    muted: bool,
    role: SourceRole,
    resampler: ToPipelineRate,
    buf: Vec<f32>,
}

pub struct Mixer {
    pub frames: Receiver<MixFrame>,
    pub commands: Sender<MixerCommand>,
}

impl Mixer {
    pub fn start(sources: &[MixerSource]) -> Result<Self> {
        let (frame_tx, frame_rx) = bounded::<MixFrame>(64);
        let (cmd_tx, cmd_rx) = bounded::<MixerCommand>(32);
        let (ready_tx, ready_rx) = bounded::<Result<(), String>>(1);

        let sources = sources.to_vec();
        thread::Builder::new().name("learnlive-mixer".into()).spawn(move || {
            // Opened here so the `!Send` streams live exactly as long as this thread.
            let (_captures, live) = match open_sources(&sources) {
                Ok(v) => {
                    let _ = ready_tx.send(Ok(()));
                    v
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("{e:#}")));
                    return;
                }
            };
            run(live, cmd_rx, frame_tx);
        })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self { frames: frame_rx, commands: cmd_tx }),
            Ok(Err(e)) => Err(anyhow!(e)),
            Err(_) => Err(anyhow!("mixer thread died during start-up")),
        }
    }
}

/// Opens every source. Runs on the mixer thread. Returns the captures to keep alive plus the
/// per-source mixing state, and the receiver the capture callbacks feed.
#[allow(clippy::type_complexity)]
fn open_sources(sources: &[MixerSource]) -> Result<(Vec<Capture>, LiveSet)> {
    let (raw_tx, raw_rx) = bounded::<RawChunk>(256);
    let mut captures = vec![];
    let mut map: HashMap<String, Live> = HashMap::new();

    for s in sources {
        let (rate, ch) = if let Some(path) = s.device_id.strip_prefix("file:") {
            // WAV standing in for a device (demo/test). Paced at real time so VAD timing matches.
            let wav = super::file_source::read_wav(std::path::Path::new(path))?;
            let (rate, ch) = (wav.sample_rate, wav.channels);
            let (id, tx) = (s.device_id.clone(), raw_tx.clone());
            thread::spawn(move || {
                let _ = super::file_source::feed(id, wav, tx, true);
            });
            (rate, ch)
        } else {
            let cap = Capture::start(&s.device_id, raw_tx.clone())?;
            let r = (cap.sample_rate, cap.channels);
            captures.push(cap);
            r
        };
        map.insert(
            s.device_id.clone(),
            Live { gain: s.gain, muted: s.muted, role: s.role, resampler: ToPipelineRate::new(rate, ch)?, buf: vec![] },
        );
    }
    info!("mixer started with {} source(s)", sources.len());
    Ok((captures, LiveSet { map, raw: raw_rx }))
}

struct LiveSet {
    map: HashMap<String, Live>,
    raw: Receiver<RawChunk>,
}

fn run(mut live: LiveSet, cmd_rx: Receiver<MixerCommand>, frame_tx: Sender<MixFrame>) {
    let mut duck = 1.0f32;
    loop {
        // Drain commands without blocking.
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                MixerCommand::SetGain { device_id, gain } => {
                    if let Some(s) = live.map.get_mut(&device_id) {
                        s.gain = gain;
                    }
                }
                MixerCommand::SetMuted { device_id, muted } => {
                    if let Some(s) = live.map.get_mut(&device_id) {
                        s.muted = muted;
                    }
                }
                MixerCommand::Duck(d) => duck = d,
                MixerCommand::Stop => return,
            }
        }

        let Ok(chunk) = live.raw.recv_timeout(Duration::from_millis(50)) else { continue };
        if let Some(s) = live.map.get_mut(&chunk.device_id) {
            match s.resampler.push(&chunk.samples) {
                Ok(mono) => s.buf.extend(mono),
                Err(e) => error!("resample: {e}"),
            }
        }

        // Emit as many full frames as every non-empty source can supply.
        while let Some(frame) = next_frame(&mut live.map, duck) {
            if frame_tx.try_send(frame).is_err() {
                // consumer is behind; drop the frame rather than grow unbounded
            }
        }
    }
}

fn next_frame(live: &mut HashMap<String, Live>, duck: f32) -> Option<MixFrame> {
    let ready = live.values().filter(|s| !s.buf.is_empty()).all(|s| s.buf.len() >= FRAME_SAMPLES)
        && live.values().any(|s| s.buf.len() >= FRAME_SAMPLES);
    if !ready {
        return None;
    }

    let mut mix = vec![0.0f32; FRAME_SAMPLES];
    let mut rms = vec![];
    let mut loudest: Option<(SourceRole, f32)> = None;
    for (id, s) in live.iter_mut() {
        if s.buf.len() < FRAME_SAMPLES {
            continue;
        }
        let frame: Vec<f32> = s.buf.drain(..FRAME_SAMPLES).collect();
        let g = if s.muted { 0.0 } else { s.gain * if s.role == SourceRole::Remote { duck } else { 1.0 } };
        let mut energy = 0.0;
        for (m, x) in mix.iter_mut().zip(frame.iter()) {
            let v = x * g;
            *m += v;
            energy += v * v;
        }
        let r = (energy / FRAME_SAMPLES as f32).sqrt();
        rms.push((id.clone(), r));
        if loudest.map(|(_, lr)| r > lr).unwrap_or(true) {
            loudest = Some((s.role, r));
        }
    }
    for m in mix.iter_mut() {
        *m = m.clamp(-1.0, 1.0);
    }
    let dominant = loudest.filter(|(_, r)| *r > 0.005).map(|(role, _)| role);
    Some(MixFrame { mix, rms, dominant })
}

#[cfg(test)]
mod tests {
    //! Mixer maths without devices: drive the same summing code the thread runs.
    use super::*;

    fn live(frames: &[(&[f32], f32, bool, SourceRole)]) -> HashMap<String, Live> {
        frames
            .iter()
            .enumerate()
            .map(|(i, (pcm, gain, muted, role))| {
                let mut buf = pcm.to_vec();
                buf.resize(FRAME_SAMPLES, 0.0);
                (
                    format!("dev{i}"),
                    Live {
                        gain: *gain,
                        muted: *muted,
                        role: *role,
                        resampler: ToPipelineRate::new(16_000, 1).unwrap(),
                        buf,
                    },
                )
            })
            .collect()
    }

    fn mix(frames: &[(&[f32], f32, bool, SourceRole)], duck: f32) -> (Vec<f32>, Option<SourceRole>) {
        let f = next_frame(&mut live(frames), duck).expect("a full frame");
        (f.mix, f.dominant)
    }

    #[test]
    fn gain_mute_and_duck() {
        let a = [0.5f32; FRAME_SAMPLES];
        let b = [0.2f32; FRAME_SAMPLES];
        let (m, dom) = mix(&[(&a, 1.0, false, SourceRole::Remote), (&b, 1.0, false, SourceRole::Local)], 1.0);
        assert!((m[0] - 0.7).abs() < 1e-6);
        assert_eq!(dom, Some(SourceRole::Remote));
        let (m, dom) = mix(&[(&a, 1.0, true, SourceRole::Remote), (&b, 1.0, false, SourceRole::Local)], 1.0);
        assert!((m[0] - 0.2).abs() < 1e-6);
        assert_eq!(dom, Some(SourceRole::Local));
        let (m, _) = mix(&[(&a, 1.0, false, SourceRole::Remote), (&b, 1.0, false, SourceRole::Local)], 0.5);
        assert!((m[0] - 0.45).abs() < 1e-6, "remote ducked to half, local untouched");
        let (m, _) = mix(&[(&a, 2.0, false, SourceRole::Remote), (&a, 2.0, false, SourceRole::Local)], 1.0);
        assert_eq!(m[0], 1.0, "clamped");
    }

    #[test]
    fn partial_buffers_do_not_emit() {
        let short = [0.5f32; 10];
        let mut l = live(&[(&short, 1.0, false, SourceRole::Remote)]);
        l.get_mut("dev0").unwrap().buf.truncate(10);
        assert!(next_frame(&mut l, 1.0).is_none(), "a sub-frame buffer must wait for more audio");
    }
}
