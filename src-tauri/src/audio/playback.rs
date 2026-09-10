//! Output to the default speakers (TTS + clip replay). Simple queue-based cpal output stream.
//!
//! `cpal::Stream` is `!Send + !Sync`, so it can never live in a struct that Tauri holds as
//! managed state. The stream is therefore owned by a dedicated thread and `Player` is just a
//! handle to the sample queue it drains — which *is* `Send + Sync`. Dropping the handle stops
//! the thread.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::bounded;
use log::warn;
use parking_lot::Mutex;

type Queue = Arc<Mutex<VecDeque<f32>>>;

pub struct Player {
    queue: Queue,
    rate: u32,
    stop: Arc<AtomicBool>,
}

impl Player {
    pub fn open() -> Result<Self> {
        let queue: Queue = Arc::new(Mutex::new(VecDeque::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = bounded::<Result<u32, String>>(1);

        let (q, s) = (queue.clone(), stop.clone());
        thread::Builder::new().name("learnlive-out".into()).spawn(move || match build(q) {
            Ok((stream, rate)) => {
                let _ = tx.send(Ok(rate));
                // Hold the stream here; it is dropped (stopping playback) when the handle goes.
                while !s.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(50));
                }
                drop(stream);
            }
            Err(e) => {
                let _ = tx.send(Err(format!("{e:#}")));
            }
        })?;

        match rx.recv() {
            Ok(Ok(rate)) => Ok(Self { queue, rate, stop }),
            Ok(Err(e)) => Err(anyhow!(e)),
            Err(_) => Err(anyhow!("audio output thread died during start-up")),
        }
    }

    /// Queue mono samples at `src_rate` (naive linear resample to device rate — fine for speech).
    pub fn play(&self, mono: &[f32], src_rate: u32) {
        if mono.is_empty() || src_rate == 0 {
            return;
        }
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

    /// How long the queued audio will take to drain, at the device rate.
    pub fn queued_duration(&self) -> Duration {
        Duration::from_secs_f32(self.queue.lock().len() as f32 / self.rate as f32)
    }

    pub fn is_playing(&self) -> bool {
        !self.queue.lock().is_empty()
    }

    pub fn clear(&self) {
        self.queue.lock().clear();
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Runs on the audio thread: everything here touches `!Send` cpal types.
fn build(queue: Queue) -> Result<(cpal::Stream, u32)> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| anyhow!("no output device"))?;
    let cfg = device.default_output_config()?;
    let rate = cfg.sample_rate().0;
    let channels = cfg.channels() as usize;
    let stream = device.build_output_stream(
        &cfg.into(),
        move |out: &mut [f32], _| {
            let mut q = queue.lock();
            for frame in out.chunks_mut(channels) {
                let s = q.pop_front().unwrap_or(0.0);
                for c in frame.iter_mut() {
                    *c = s;
                }
            }
        },
        |e| warn!("output stream: {e}"),
        None,
    )?;
    stream.play()?;
    Ok((stream, rate))
}
