import { useState } from 'react'
import { speakerColor } from '../lib/pos'
import type { Segment } from '../lib/types'

interface Props { segments: Segment[]; onRename: (id: number, label: string) => void }

export default function Speakers({ segments, onRename }: Props) {
  const [editing, setEditing] = useState<number | null>(null)
  const seen = new Map<number, { label: string; count: number }>()
  for (const s of segments) {
    const e = seen.get(s.speaker.id)
    seen.set(s.speaker.id, { label: s.speaker.label, count: (e?.count ?? 0) + 1 })
  }
  if (seen.size === 0) return null
  return (
    <section className="speakers" aria-label="Speakers">
      <h2>Who's talking</h2>
      <ul>
        {[...seen.entries()].sort((a, b) => a[0] - b[0]).map(([id, { label, count }]) => (
          <li key={id}>
            <span className="swatch" style={{ background: speakerColor(id) }} />
            {editing === id
              ? <input autoFocus defaultValue={label} onBlur={e => { onRename(id, e.target.value || label); setEditing(null) }} onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur() }} />
              : <button className="linklike" onClick={() => setEditing(id)} title="Rename">{label}</button>}
            <small>{count}</small>
          </li>
        ))}
      </ul>
    </section>
  )
}
