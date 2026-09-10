import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AudioDevice,
  CalendarEvent,
  Language,
  Levels,
  MeetingSummary,
  MixerSource,
  ModelProgress,
  ModelStatus,
  SearchHit,
  Segment,
  SessionConfig,
} from './types'

export const api = {
  devices: () => invoke<AudioDevice[]>('list_audio_devices'),
  languages: () => invoke<Language[]>('list_languages'),
  modelStatus: (cfg: SessionConfig) => invoke<ModelStatus[]>('model_status', { cfg }),
  prepareModels: (cfg: SessionConfig) => invoke<void>('prepare_models', { cfg }),
  start: (cfg: SessionConfig, event: CalendarEvent | null) => invoke<number>('start_session', { cfg, event }),
  stop: () => invoke<void>('stop_session'),
  updateMixer: (source: MixerSource) => invoke<void>('update_mixer', { source }),
  renameSpeaker: (id: number, label: string) => invoke<void>('rename_speaker', { id, label }),
  speak: (text: string, lang: string, speed = 1) => invoke<void>('speak', { text, lang, speed }),
  playClip: (path: string) => invoke<void>('play_clip', { path }),

  calendarNearNow: () => invoke<CalendarEvent[]>('calendar_events_near_now'),
  linkMeeting: (meetingId: number, event: CalendarEvent) => invoke<MeetingSummary>('link_meeting', { meetingId, event }),
  assignSpeaker: (meetingId: number, speakerId: number, name: string, email: string | null) =>
    invoke<MeetingSummary>('assign_speaker', { meetingId, speakerId, name, email }),
  listMeetings: (limit?: number) => invoke<MeetingSummary[]>('list_meetings', { limit }),
  getMeeting: (id: number) => invoke<[MeetingSummary, Segment[]] | null>('get_meeting', { id }),
  search: (query: string, limit?: number) => invoke<SearchHit[]>('search_history', { query, limit }),
  deleteMeeting: (id: number) => invoke<void>('delete_meeting', { id }),
  getRememberVoices: () => invoke<boolean>('get_remember_voices'),
  setRememberVoices: (on: boolean) => invoke<void>('set_remember_voices', { on }),
  forgetVoice: (participantId: number) => invoke<void>('forget_voice', { participantId }),

  onSegment: (cb: (s: Segment) => void): Promise<UnlistenFn> => listen<Segment>('segment', e => cb(e.payload)),
  onLevels: (cb: (l: Levels) => void): Promise<UnlistenFn> => listen<Levels>('levels', e => cb(e.payload)),
  onModelProgress: (cb: (p: ModelProgress) => void): Promise<UnlistenFn> => listen<ModelProgress>('model-progress', e => cb(e.payload)),
  onError: (cb: (msg: string) => void): Promise<UnlistenFn> => listen<string>('pipeline-error', e => cb(e.payload)),
}
