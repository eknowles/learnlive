import { describe, expect, it } from 'vitest'
import { changedIndices } from '../lib/diff'
import type { Token } from '../lib/types'

const tok = (text: string): Token => ({ text, lemma: text, pos: 'NOUN', feats: {}, aligned_to: null })
const t = (s: string) => s.split(' ').map(tok)

describe('changedIndices', () => {
  it('flags nothing on first draft', () => {
    expect(changedIndices(undefined, t('я читаю'))).toEqual(new Set())
  })
  it('flags appended words', () => {
    expect(changedIndices(t('я читаю'), t('я читаю книгу дома'))).toEqual(new Set([2, 3]))
  })
  it('flags a changed case ending but not a moved word', () => {
    // "книга" → "книгу" changed form; "дома" moved earlier but is unchanged
    expect(changedIndices(t('я читаю книга дома'), t('дома я читаю книгу'))).toEqual(new Set([3]))
  })
  it('handles repeated words as a multiset', () => {
    expect(changedIndices(t('да да'), t('да да да'))).toEqual(new Set([2]))
  })
})
