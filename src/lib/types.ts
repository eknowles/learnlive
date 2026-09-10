export type SourceRole = 'remote' | 'local'

export interface AudioDevice {
  id: string
  name: string
  input_channels: number
  default_sample_rate: number
  is_loopback: boolean
}

export interface MixerSource {
  device_id: string
  role: SourceRole
  gain: number
  muted: boolean
}

export interface SessionConfig {
  sources: MixerSource[]
  learning: string
  native: string
  source_lang: string
  speak_translations: boolean
  duck_amount: number
  asr_model: 'tiny' | 'base' | 'small' | 'medium' | 'large-v3-turbo'
  diarize: boolean
}

export interface Language {
  code: string
  name: string
  native: string
  nllb: string
  tts: boolean
  grammar: boolean
}

export interface Token {
  text: string
  lemma: string
  pos: string
  feats: Record<string, string>
  aligned_to: [number, number] | null
}

export interface SpeakerRef {
  id: number
  label: string
  confidence: number
}

export interface Segment {
  id: string
  speaker: SpeakerRef
  role: SourceRole
  started_ms: number
  ended_ms: number
  source_lang: string
  source_text: string
  target_lang: string
  target_text: string
  tokens: Token[]
  clip_path: string | null
  final: boolean
  revision: number
  arrived_at: number
}

export interface ModelStatus {
  id: string
  kind: string
  present: boolean
  approx_mb: number
}
export interface ModelProgress {
  model: string
  bytes: number
  total: number | null
  done: boolean
}
export interface Levels {
  per_source: [string, number][]
  mix: number
  speech_active: boolean
}

export interface Attendee {
  name: string
  email: string
}
export interface CalendarEvent {
  id: string
  title: string
  start: number
  end: number
  attendees: Attendee[]
  url: string | null
  calendar: string
}
export interface ParticipantRef {
  id: number
  name: string
  email: string | null
  speaker_id: number | null
  has_voiceprint: boolean
}
export interface MeetingSummary {
  id: number
  title: string
  started_at: number
  ended_at: number | null
  learning: string
  native: string
  calendar_event_id: string | null
  participants: ParticipantRef[]
  sentence_count: number
}
export interface SearchHit {
  meeting_id: number
  meeting_title: string
  started_at: number
  segment: Segment
}
