import { describe, expect, it } from 'vitest'
import { toTurns } from '../components/Transcript'
import type { Segment } from '../lib/types'

let n = 0
const seg = (speakerId: number, startedMs: number, endedMs = startedMs + 1000): Segment => ({
  id: `s${n++}`,
  speaker: { id: speakerId, label: `Speaker ${speakerId}`, confidence: 0.9 },
  role: 'remote',
  started_ms: startedMs,
  ended_ms: endedMs,
  source_lang: 'ru',
  source_text: 'привет',
  target_lang: 'en',
  target_text: 'hello',
  tokens: [],
  clip_path: null,
  final: true,
  revision: 0,
  arrived_at: 0,
})

describe('toTurns', () => {
  it('runs consecutive sentences from one speaker into a single turn', () => {
    const turns = toTurns([seg(1, 0), seg(1, 1200), seg(1, 2400)])
    expect(turns).toHaveLength(1)
    expect(turns[0].rows).toHaveLength(3)
  })
  it('starts a new turn when the speaker changes', () => {
    const turns = toTurns([seg(1, 0), seg(2, 1200), seg(1, 2400)])
    expect(turns.map(t => t.speaker.id)).toEqual([1, 2, 1])
  })
  it('breaks a turn on a long silence, even from the same speaker', () => {
    // `gap` only reports a pause at 45 s or more.
    const turns = toTurns([seg(1, 0, 1000), seg(1, 90_000)])
    expect(turns).toHaveLength(2)
    expect(turns[1].pause).toBe('89 s pause')
  })
  it('does not break on an ordinary breath', () => {
    const turns = toTurns([seg(1, 0, 1000), seg(1, 3000)])
    expect(turns).toHaveLength(1)
    expect(turns[0].pause).toBeNull()
  })
  it('has no pause before the very first turn', () => {
    expect(toTurns([seg(1, 5000)])[0].pause).toBeNull()
  })
})
