//! Mono downmix + resampling to the pipeline rate using rubato's sinc resampler.

use anyhow::Result;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

use super::PIPELINE_RATE;

pub struct ToPipelineRate {
    inner: Option<SincFixedIn<f32>>,
    channels: usize,
    pending: Vec<f32>,
    chunk: usize,
}

impl ToPipelineRate {
    pub fn new(src_rate: u32, channels: u16) -> Result<Self> {
        let inner = if src_rate == PIPELINE_RATE {
            None
        } else {
            let params = SincInterpolationParameters {
                sinc_len: 128,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 128,
                window: WindowFunction::BlackmanHarris2,
            };
            Some(SincFixedIn::new(PIPELINE_RATE as f64 / src_rate as f64, 2.0, params, 1024, 1)?)
        };
        Ok(Self { inner, channels: channels as usize, pending: vec![], chunk: 1024 })
    }

    /// Feed interleaved device samples; returns any 16 kHz mono samples now available.
    pub fn push(&mut self, interleaved: &[f32]) -> Result<Vec<f32>> {
        // Downmix to mono.
        let mono = interleaved.chunks(self.channels).map(|f| f.iter().sum::<f32>() / self.channels as f32);
        self.pending.extend(mono);

        let Some(rs) = self.inner.as_mut() else {
            return Ok(std::mem::take(&mut self.pending));
        };

        let mut out = vec![];
        while self.pending.len() >= self.chunk {
            let block: Vec<f32> = self.pending.drain(..self.chunk).collect();
            let res = rs.process(&[block], None)?;
            out.extend_from_slice(&res[0]);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stereo_48k_to_mono_16k_keeps_duration() {
        let n = 48_000; // 1 s
        let inter: Vec<f32> = (0..n).flat_map(|i| { let t = i as f32 / 48_000.0; let s = (t * 440.0 * std::f32::consts::TAU).sin() * 0.5; [s, s] }).collect();
        let mut rs = ToPipelineRate::new(48_000, 2).unwrap();
        let out = rs.push(&inter).unwrap();
        // Resampler holds a partial block; expect ~1 s minus < 1 chunk.
        assert!(out.len() > 15_000 && out.len() <= 16_000, "got {} samples", out.len());
        let rms = (out.iter().map(|x| x * x).sum::<f32>() / out.len() as f32).sqrt();
        assert!((rms - 0.5 / 2f32.sqrt()).abs() < 0.05, "level preserved, rms={rms}");
    }
    #[test]
    fn passthrough_at_16k() {
        let mut rs = ToPipelineRate::new(16_000, 1).unwrap();
        assert_eq!(rs.push(&[0.1; 320]).unwrap().len(), 320);
    }
}
