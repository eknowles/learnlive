import { describe, expect, it } from 'vitest'
import { ago, gap } from '../lib/time'

describe('ago', () => {
  const now = 1_000_000_000
  it('rounds coarsely', () => {
    expect(ago(now - 2_000, now)).toBe('just now')
    expect(ago(now - 12_000, now)).toBe('12 s ago')
    expect(ago(now - 3 * 60_000, now)).toBe('3 min ago')
    expect(ago(now - 65 * 60_000, now)).toBe('1 h 05 ago')
  })
  it('never goes negative when clocks skew', () => {
    expect(ago(now + 5_000, now)).toBe('just now')
  })
})

describe('gap', () => {
  it('ignores short pauses', () => expect(gap(0, 30_000)).toBeNull())
  it('formats seconds then minutes', () => {
    expect(gap(0, 50_000)).toBe('50 s pause')
    expect(gap(0, 180_000)).toBe('3 min pause')
  })
})
