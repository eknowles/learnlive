use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;
use log::{info, warn};

use crate::types::AudioDevice;

const LOOPBACK_HINTS: &[&str] = &["blackhole", "loopback", "aggregate", "soundflower", "vb-cable", "monitor of"];

/// Enumerate input devices. IDs are the device names (cpal has no stable ID), which is fine on macOS.
pub fn list_devices() -> Result<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let mut out = vec![];
    for dev in host.input_devices()? {
        let name = dev.name().unwrap_or_else(|_| "Unknown".into());
        let cfg = match dev.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                warn!("skipping {name}: {e}");
                continue;
            }
        };
        let lower = name.to_lowercase();
        out.push(AudioDevice {
            id: name.clone(),
            name,
            input_channels: cfg.channels(),
            default_sample_rate: cfg.sample_rate().0,
            is_loopback: LOOPBACK_HINTS.iter().any(|h| lower.contains(h)),
        });
    }
    Ok(out)
}

/// Raw audio from one device: interleaved samples at the device's native rate/channels.
pub struct RawChunk {
    pub device_id: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

/// A running cpal stream. Dropping it stops capture.
pub struct Capture {
    _stream: cpal::Stream,
    pub device_id: String,
    pub sample_rate: u32,
    pub channels: u16,
}

impl Capture {
    pub fn start(device_id: &str, tx: Sender<RawChunk>) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .input_devices()?
            .find(|d| d.name().map(|n| n == device_id).unwrap_or(false))
            .ok_or_else(|| anyhow!("audio device not found: {device_id}"))?;

        let cfg = device.default_input_config().context("default input config")?;
        let sample_rate = cfg.sample_rate().0;
        let channels = cfg.channels();
        let id = device_id.to_string();
        info!("capturing {id} @ {sample_rate} Hz, {channels} ch, {:?}", cfg.sample_format());

        let err_fn = move |e| warn!("stream error: {e}");
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => {
                let id = id.clone();
                device.build_input_stream(
                    &cfg.into(),
                    move |data: &[f32], _| {
                        let _ = tx.try_send(RawChunk { device_id: id.clone(), sample_rate, channels, samples: data.to_vec() });
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let id = id.clone();
                device.build_input_stream(
                    &cfg.into(),
                    move |data: &[i16], _| {
                        let samples = data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                        let _ = tx.try_send(RawChunk { device_id: id.clone(), sample_rate, channels, samples });
                    },
                    err_fn,
                    None,
                )?
            }
            other => return Err(anyhow!("unsupported sample format {other:?}")),
        };
        stream.play()?;
        Ok(Self { _stream: stream, device_id: id, sample_rate, channels })
    }
}
