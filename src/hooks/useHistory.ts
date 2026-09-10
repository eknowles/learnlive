import { useCallback, useEffect, useState } from 'react'
import { api } from '../lib/api'
import type { MeetingSummary, SearchHit, Segment } from '../lib/types'

export interface OpenMeeting {
  meeting: MeetingSummary
  segments: Segment[]
}

/** Past meetings for the sidebar, the one that's open, and full-text search across all of them. */
export function useHistory() {
  const [meetings, setMeetings] = useState<MeetingSummary[]>([])
  const [open, setOpen] = useState<OpenMeeting | null>(null)
  const [query, setQuery] = useState('')
  const [hits, setHits] = useState<SearchHit[]>([])

  const refresh = useCallback(() => api.listMeetings().then(setMeetings), [])
  useEffect(() => {
    refresh()
  }, [refresh])

  useEffect(() => {
    if (!query.trim()) {
      setHits([])
      return
    }
    const t = setTimeout(
      () =>
        api
          .search(query)
          .then(setHits)
          .catch(() => setHits([])),
      150,
    )
    return () => clearTimeout(t)
  }, [query])

  const openMeeting = useCallback(async (id: number) => {
    const r = await api.getMeeting(id)
    setOpen(r ? { meeting: r[0], segments: r[1] } : null)
    return r !== null
  }, [])

  const close = useCallback(() => setOpen(null), [])

  const remove = useCallback(
    async (id: number) => {
      await api.deleteMeeting(id)
      setOpen(o => (o?.meeting.id === id ? null : o))
      await refresh()
    },
    [refresh],
  )

  return { meetings, refresh, open, openMeeting, close, remove, query, setQuery, hits }
}
