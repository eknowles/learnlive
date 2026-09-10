import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { AudioDevice, Language, Levels, MixerSource, ModelProgress, ModelStatus, Segment, SessionConfig } from './types'

export const api = {
  devices: () => invoke<AudioDevice[]>('list_audio_devices'),
  languages: () => invoke<Language[]>('list_languages'),
  modelStatus: (cfg: SessionConfig) => invoke<ModelStatus[]>('model_status', { cfg }),
  prepareModels: (cfg: SessionConfig) => invoke<void>('prepare_models', { cfg }),
  start: (cfg: SessionConfig) => invoke<void>('start_session', { cfg }),
  stop: () => invoke<void>('stop_session'),
  updateMixer: (source: MixerSource) => invoke<void>('update_mixer', { source }),
  renameSpeaker: (id: number, label: string) => invoke<void>('rename_speaker', { id, label }),
  speak: (text: string, lang: string, speed = 1) => invoke<void>('speak', { text, lang, speed }),
  playClip: (path: string) => invoke<void>('play_clip', { path }),

  onSegment: (cb: (s: Segment) => void): Promise<UnlistenFn> => listen<Segment>('segment', e => cb(e.payload)),
  onLevels: (cb: (l: Levels) => void): Promise<UnlistenFn> => listen<Levels>('levels', e => cb(e.payload)),
  onModelProgress: (cb: (p: ModelProgress) => void): Promise<UnlistenFn> => listen<ModelProgress>('model-progress', e => cb(e.payload)),
  onError: (cb: (msg: string) => void): Promise<UnlistenFn> => listen<string>('pipeline-error', e => cb(e.payload)),
}
