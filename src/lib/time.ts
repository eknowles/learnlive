export const clock = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })

/** "just now", "12 s ago", "3 min ago", "1 h 05 ago" — coarse on purpose so it doesn't flicker. */
export const ago = (ms: number, now: number) => {
  const s = Math.max(0, Math.round((now - ms) / 1000))
  if (s < 5) return 'just now'
  if (s < 60) return `${s} s ago`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m} min ago`
  const h = Math.floor(m / 60)
  return `${h} h ${String(m % 60).padStart(2, '0')} ago`
}

/** Length of a silence between two lines, if worth showing. */
export const gap = (prevEnd: number, nextStart: number) => {
  const s = Math.round((nextStart - prevEnd) / 1000)
  if (s < 45) return null
  return s < 120 ? `${s} s pause` : `${Math.round(s / 60)} min pause`
}
