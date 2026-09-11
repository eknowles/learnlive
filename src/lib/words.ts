/** Splitting a sentence into hoverable words.
 *
 *  The POS tagger would give us tokens, but it has no working model source (see the README's
 *  known debt), so `segment.tokens` is empty in practice and the reading surface has to do its
 *  own splitting. When a tagger returns, `Transcript` prefers its tokens and this is unused. */

export interface Span {
  text: string
  /** Offset into the sentence, in UTF-16 code units — what JS slicing uses.
   *  A future aligner reports *byte* ranges from Rust, so it will need converting. */
  start: number
  end: number
  /** False for the punctuation and whitespace between words. */
  word: boolean
}

// Letters and digits, allowing an internal apostrophe or hyphen ("don't", "well-known").
const WORD = /[\p{L}\p{N}]+(?:[''’-][\p{L}\p{N}]+)*/gu

export const splitWords = (text: string): Span[] => {
  const out: Span[] = []
  let at = 0
  for (const m of text.matchAll(WORD)) {
    const start = m.index
    if (start > at) out.push({ text: text.slice(at, start), start: at, end: start, word: false })
    out.push({ text: m[0], start, end: start + m[0].length, word: true })
    at = start + m[0].length
  }
  if (at < text.length) out.push({ text: text.slice(at), start: at, end: text.length, word: false })
  return out
}

/** Just the words, for diffing one revision against the last. */
export const wordsOf = (text: string): string[] =>
  splitWords(text)
    .filter(s => s.word)
    .map(s => s.text)
