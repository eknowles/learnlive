import { useEffect, useMemo, useState } from 'react'
import { api } from '../lib/api'
import type { Segment } from '../lib/types'

/** Live segments with upsert-by-id (revisions replace drafts) and local speaker renames. */
export function useTranscript() {
  const [segments, setSegments] = useState<Segment[]>([])
  const [names, setNames] = useState<Record<number, string>>({})

  useEffect(() => {
    const p = api.onSegment(s =>
      setSegments(prev => (prev.some(x => x.id === s.id) ? prev.map(x => (x.id === s.id ? s : x)) : [...prev, s])),
    )
    return () => {
      p.then(f => f())
    }
  }, [])

  const shown = useMemo(
    () => segments.map(s => ({ ...s, speaker: { ...s.speaker, label: names[s.speaker.id] ?? s.speaker.label } })),
    [segments, names],
  )
  const rename = (id: number, label: string) => setNames(n => ({ ...n, [id]: label }))
  const reset = () => {
    setSegments([])
    setNames({})
  }
  return { segments: shown, rename, reset }
}
