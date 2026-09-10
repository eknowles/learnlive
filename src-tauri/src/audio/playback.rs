//! Output to the default speakers (TTS + clip replay). Simple queue-based cpal output stream.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use log::warn;

pub struct Player {
    _stream: cpal::Stream,
    queue: Arc<Mutex<std::collections::VecDeque<f32>>>,
    rate: u32,
    channels: usize,
}

impl Player {
    pub fn open() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or_else(|| anyhow!("no output device"))?;
        let cfg = device.default_output_config()?;
        let rate = cfg.sample_rate().0;
        let channels = cfg.channels() as usize;
        let queue = Arc::new(Mutex::new(std::collections::VecDeque::<f32>::new()));
        let q = queue.clone();
        let stream = device.build_output_stream(
            &cfg.into(),
            move |out: &mut [f32], _| {
                let mut q = q.lock();
                for frame in out.chunks_mut(channels) {
                    let s = q.pop_front().unwrap_or(0.0);
                    for c in frame.iter_mut() { *c = s; }
                }
            },
            |e| warn!("output stream: {e}"),
            None,
        )?;
        stream.play()?;
        Ok(Self { _stream: stream, queue, rate, channels })
    }

    /// Queue mono samples at `src_rate` (naive linear resample to device rate — fine for speech).
    pub fn play(&self, mono: &[f32], src_rate: u32) {
        let ratio = self.rate as f32 / src_rate as f32;
        let n = (mono.len() as f32 * ratio) as usize;
        let mut q = self.queue.lock();
        for i in 0..n {
            let pos = i as f32 / ratio;
            let a = mono[(pos as usize).min(mono.len() - 1)];
            let b = mono[((pos as usize) + 1).min(mono.len() - 1)];
            q.push_back(a + (b - a) * pos.fract());
        }
    }

    pub fn is_playing(&self) -> bool { !self.queue.lock().is_empty() }
    pub fn clear(&self) { self.queue.lock().clear(); }
}
