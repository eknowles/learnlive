//! Sums N resampled sources into one 16 kHz stream, in fixed 20 ms frames.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use log::{error, info};
use parking_lot::RwLock;

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
    SetGain { device_id: String, gain: f32 },
    SetMuted { device_id: String, muted: bool },
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
    _captures: Vec<Capture>,
}

impl Mixer {
    pub fn start(sources: &[MixerSource]) -> Result<Self> {
        let (raw_tx, raw_rx) = bounded::<RawChunk>(256);
        let (frame_tx, frame_rx) = bounded::<MixFrame>(64);
        let (cmd_tx, cmd_rx) = bounded::<MixerCommand>(32);

        let live: Arc<RwLock<HashMap<String, Live>>> = Arc::new(RwLock::new(HashMap::new()));
        let mut captures = vec![];
        for s in sources {
            let cap = Capture::start(&s.device_id, raw_tx.clone())?;
            live.write().insert(
                s.device_id.clone(),
                Live {
                    gain: s.gain,
                    muted: s.muted,
                    role: s.role,
                    resampler: ToPipelineRate::new(cap.sample_rate, cap.channels)?,
                    buf: vec![],
                },
            );
            captures.push(cap);
        }
        info!("mixer started with {} source(s)", captures.len());

        let live2 = live.clone();
        thread::Builder::new().name("learnlive-mixer".into()).spawn(move || {
            let mut duck = 1.0f32;
            loop {
                // Drain commands without blocking.
                while let Ok(cmd) = cmd_rx.try_recv() {
                    let mut l = live2.write();
                    match cmd {
                        MixerCommand::SetGain { device_id, gain } => { if let Some(s) = l.get_mut(&device_id) { s.gain = gain; } }
                        MixerCommand::SetMuted { device_id, muted } => { if let Some(s) = l.get_mut(&device_id) { s.muted = muted; } }
                        MixerCommand::Duck(d) => duck = d,
                        MixerCommand::Stop => return,
                    }
                }

                let Ok(chunk) = raw_rx.recv_timeout(std::time::Duration::from_millis(50)) else { continue };
                {
                    let mut l = live2.write();
                    if let Some(s) = l.get_mut(&chunk.device_id) {
                        match s.resampler.push(&chunk.samples) {
                            Ok(mono) => s.buf.extend(mono),
                            Err(e) => error!("resample: {e}"),
                        }
                    }
                }

                // Emit as many full frames as every non-empty source can supply.
                loop {
                    let mut l = live2.write();
                    let ready = l.values().filter(|s| !s.buf.is_empty()).all(|s| s.buf.len() >= FRAME_SAMPLES)
                        && l.values().any(|s| s.buf.len() >= FRAME_SAMPLES);
                    if !ready { break; }

                    let mut mix = vec![0.0f32; FRAME_SAMPLES];
                    let mut rms = vec![];
                    let mut loudest: Option<(SourceRole, f32)> = None;
                    for (id, s) in l.iter_mut() {
                        if s.buf.len() < FRAME_SAMPLES { continue; }
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
                        if loudest.map(|(_, lr)| r > lr).unwrap_or(true) { loudest = Some((s.role, r)); }
                    }
                    for m in mix.iter_mut() { *m = m.clamp(-1.0, 1.0); }
                    let dominant = loudest.filter(|(_, r)| *r > 0.005).map(|(role, _)| role);
                    if frame_tx.try_send(MixFrame { mix, rms, dominant }).is_err() {
                        // consumer is behind; drop the frame rather than grow unbounded
                    }
                }
            }
        })?;

        Ok(Self { frames: frame_rx, commands: cmd_tx, _captures: captures })
    }
}
