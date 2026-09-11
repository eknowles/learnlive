import { describe, expect, it } from 'vitest'
import { POS, SPEAKER_COLORS, speakerColor } from '../lib/pos'

/** Verified against WebKit by rendering each keyword and reading back the computed colour:
 *  anything outside this set resolves to the default label colour instead of failing loudly,
 *  so a bad entry looks like "the speaker has no colour" rather than like a bug. */
const RESOLVES = new Set([
  '-apple-system-blue',
  '-apple-system-brown',
  '-apple-system-gray',
  '-apple-system-green',
  '-apple-system-orange',
  '-apple-system-pink',
  '-apple-system-purple',
  '-apple-system-red',
  '-apple-system-yellow',
])

describe('speaker colours', () => {
  it('only uses keywords WebKit actually implements', () => {
    for (const c of SPEAKER_COLORS) expect(RESOLVES, `${c} does not resolve in WebKit`).toContain(c)
  })
  it('gives consecutive speakers different colours', () => {
    const first = [1, 2, 3, 4].map(speakerColor)
    expect(new Set(first).size).toBe(4)
  })
  it('leaves you as plain label colour, so the other voices carry the hue', () => {
    expect(speakerColor(0)).toBe('var(--label)')
  })
  it('wraps round rather than running out', () => {
    expect(speakerColor(1 + SPEAKER_COLORS.length)).toBe(speakerColor(1))
  })
})

describe('part-of-speech colours', () => {
  it('only uses keywords WebKit actually implements', () => {
    for (const [tag, { color }] of Object.entries(POS)) {
      if (color === 'transparent') continue
      expect(RESOLVES, `${tag} uses ${color}`).toContain(color)
    }
  })
})
