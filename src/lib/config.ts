import type { AudioDevice, MixerSource, SessionConfig } from './types'

export const DEFAULT_CONFIG: SessionConfig = {
  sources: [],
  learning: 'ru',
  native: 'en',
  source_lang: 'auto',
  speak_translations: true,
  duck_amount: 0.5,
  asr_model: 'small',
  diarize: true,
}

/** First loopback device as the call, first non-loopback as the mic. */
export const defaultSources = (devices: AudioDevice[]): MixerSource[] => {
  const loop = devices.find(d => d.is_loopback)
  const mic = devices.find(d => !d.is_loopback)
  return [
    ...(loop ? [{ device_id: loop.id, role: 'remote' as const, gain: 1, muted: false }] : []),
    ...(mic ? [{ device_id: mic.id, role: 'local' as const, gain: 1, muted: false }] : []),
  ]
}

/** Fields that change which models are needed (drives model_status refresh). */
export const modelKey = (c: SessionConfig) => [c.learning, c.native, c.asr_model, c.diarize, c.speak_translations].join('|')
