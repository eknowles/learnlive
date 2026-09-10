//! A WAV file standing in for a capture device. Used by offline runs and by the live mixer
//! when a source id is `file:/path.wav` (handy for demoing without a call).

use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;

use super::capture::RawChunk;

pub struct Wav {
    pub sample_rate: u32,
    pub channels: u16,
    /// Interleaved f32 in [-1, 1].
    pub samples: Vec<f32>,
}

/// Minimal PCM WAV reader (8/16/24/32-bit int and 32-bit float). Enough for fixtures and
/// exported meeting recordings; anything exotic → convert with ffmpeg first.
pub fn read_wav(path: &Path) -> Result<Wav> {
    let b = std::fs::read(path)?;
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(anyhow!("not a RIFF/WAVE file"));
    }
    let (mut pos, mut fmt, mut data) = (12usize, None, None);
    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let len = u32::from_le_bytes(b[pos + 4..pos + 8].try_into()?) as usize;
        let body = pos + 8;
        match id {
            b"fmt " => {
                fmt = Some((
                    u16::from_le_bytes(b[body..body + 2].try_into()?),       // format tag
                    u16::from_le_bytes(b[body + 2..body + 4].try_into()?),   // channels
                    u32::from_le_bytes(b[body + 4..body + 8].try_into()?),   // rate
                    u16::from_le_bytes(b[body + 14..body + 16].try_into()?), // bits
                ))
            }
            b"data" => data = Some(&b[body..(body + len).min(b.len())]),
            _ => {}
        }
        pos = body + len + (len & 1);
    }
    let (tag, channels, sample_rate, bits) = fmt.ok_or_else(|| anyhow!("no fmt chunk"))?;
    let data = data.ok_or_else(|| anyhow!("no data chunk"))?;
    let samples: Vec<f32> = match (tag, bits) {
        (1, 8) => data.iter().map(|&x| (x as f32 - 128.0) / 128.0).collect(),
        (1, 16) => data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect(),
        (1, 24) => data
            .chunks_exact(3)
            .map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.0)
            .collect(),
        (1, 32) => {
            data.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap()) as f32 / 2_147_483_648.0).collect()
        }
        (3, 32) => data.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect(),
        _ => return Err(anyhow!("unsupported WAV format tag={tag} bits={bits}")),
    };
    Ok(Wav { sample_rate, channels, samples })
}

pub fn write_wav16(path: &Path, rate: u32, channels: u16, interleaved: &[f32]) -> Result<()> {
    let data: Vec<u8> =
        interleaved.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
    let mut buf = Vec::with_capacity(44 + data.len());
    let block = channels as u32 * 2;
    buf.extend(b"RIFF");
    buf.extend(((36 + data.len()) as u32).to_le_bytes());
    buf.extend(b"WAVEfmt ");
    buf.extend(16u32.to_le_bytes());
    buf.extend(1u16.to_le_bytes());
    buf.extend(channels.to_le_bytes());
    buf.extend(rate.to_le_bytes());
    buf.extend((rate * block).to_le_bytes());
    buf.extend((block as u16).to_le_bytes());
    buf.extend(16u16.to_le_bytes());
    buf.extend(b"data");
    buf.extend((data.len() as u32).to_le_bytes());
    buf.extend(data);
    std::fs::write(path, buf)?;
    Ok(())
}

/// Feed a WAV into the mixer as if it were a device. `realtime` paces chunks at wall-clock
/// speed (for demos); `false` pushes as fast as the consumer drains (for tests/CI).
pub fn feed(device_id: String, wav: Wav, tx: Sender<RawChunk>, realtime: bool) -> Result<()> {
    let frames_per_chunk = (wav.sample_rate / 50) as usize; // 20 ms
    let step = frames_per_chunk * wav.channels as usize;
    for chunk in wav.samples.chunks(step) {
        tx.send(RawChunk {
            device_id: device_id.clone(),
            sample_rate: wav.sample_rate,
            channels: wav.channels,
            samples: chunk.to_vec(),
        })
        .map_err(|_| anyhow!("mixer went away"))?;
        if realtime {
            thread::sleep(Duration::from_millis(20));
        }
    }
    Ok(())
}
