import type { Token } from './types'

/** Indices of words present in `next` that weren't in `prev` (multiset diff, order-insensitive).
 *  Order-insensitive on purpose: a word that merely moved isn't "new", but a word whose
 *  form changed (книга → книгу) is. */
export const changedIndices = (prev: Token[] | undefined, next: Token[]): Set<number> => {
  if (!prev) return new Set()
  const before = new Map<string, number>()
  for (const t of prev) before.set(t.text, (before.get(t.text) ?? 0) + 1)
  const out = new Set<number>()
  next.forEach((t, i) => {
    const n = before.get(t.text) ?? 0
    if (n === 0) out.add(i)
    else before.set(t.text, n - 1)
  })
  return out
}
