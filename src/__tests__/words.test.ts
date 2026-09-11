import { describe, expect, it } from 'vitest'
import { splitWords, wordsOf } from '../lib/words'

describe('splitWords', () => {
  it('keeps punctuation and spacing as their own spans, so the sentence rebuilds exactly', () => {
    const text = 'Привет, как дела?'
    expect(
      splitWords(text)
        .map(s => s.text)
        .join(''),
    ).toBe(text)
  })
  it('finds words in a non-Latin script', () => {
    expect(wordsOf('Привет, как дела?')).toEqual(['Привет', 'как', 'дела'])
  })
  it('holds a word together across an apostrophe or hyphen', () => {
    expect(wordsOf("don't be well-known")).toEqual(["don't", 'be', 'well-known'])
  })
  it('reports offsets that slice the original text back out', () => {
    const text = 'Я читаю книгу'
    for (const span of splitWords(text)) expect(text.slice(span.start, span.end)).toBe(span.text)
  })
  it('has nothing to say about an empty sentence', () => {
    expect(splitWords('')).toEqual([])
    expect(wordsOf('  ')).toEqual([])
  })
})
