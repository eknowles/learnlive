/** Indices of words present in `next` that weren't in `prev` (multiset diff, order-insensitive).
 *  Order-insensitive on purpose: a word that merely moved isn't "new", but a word whose
 *  form changed (книга → книгу) is. */
export const changedIndices = (prev: string[] | undefined, next: string[]): Set<number> => {
  if (!prev) return new Set()
  const before = new Map<string, number>()
  for (const w of prev) before.set(w, (before.get(w) ?? 0) + 1)
  const out = new Set<number>()
  next.forEach((w, i) => {
    const n = before.get(w) ?? 0
    if (n === 0) out.add(i)
    else before.set(w, n - 1)
  })
  return out
}
