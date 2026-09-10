import { useEffect, useState } from 'react'
import { api } from '../lib/api'
import { clock } from '../lib/time'
import type { CalendarEvent, MeetingSummary } from '../lib/types'

interface Props {
  phase: string
  meeting: MeetingSummary | null
  pending: CalendarEvent | null                 // chosen before Start
  onPending: (e: CalendarEvent | null) => void
  onLinked: (m: MeetingSummary) => void
}

/** Which calendar event is this session? Shown before Start (pick) and during (linked). */
export default function Meeting({ phase, meeting, pending, onPending, onLinked }: Props) {
  const [events, setEvents] = useState<CalendarEvent[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = () => api.calendarNearNow().then(evs => {
    setEvents(evs)
    if (!pending && evs.length === 1 && !meeting) onPending(evs[0])
  }).catch(e => setError(String(e)))

  useEffect(() => { load() }, [])

  const choose = async (e: CalendarEvent | null) => {
    if (meeting && e) onLinked(await api.linkMeeting(meeting.id, e))
    else onPending(e)
  }

  const linked = meeting?.calendar_event_id ? meeting : null
  return (
    <section className="meeting" aria-label="Meeting">
      <h2>Meeting</h2>
      {error && <p className="quiet small">{error}</p>}
      {linked ? (
        <p className="linked"><b>{linked.title}</b><br /><span className="quiet">{clock(linked.started_at * 1000)} · {linked.participants.length} invited</span></p>
      ) : (
        <>
          {events === null && !error && <p className="quiet small">Looking at your calendar…</p>}
          {events && events.length === 0 && <p className="quiet small">Nothing on your calendar right now. The session will be saved untitled; you can attach it later.</p>}
          {events && events.length > 0 && (
            <select value={pending?.id ?? ''} onChange={e => choose(events.find(x => x.id === e.target.value) ?? null)}>
              <option value="">Not a calendar meeting</option>
              {events.map(e => <option key={e.id} value={e.id}>{e.title} · {clock(e.start * 1000).slice(0, 5)}</option>)}
            </select>
          )}
          {phase === 'live' && <button className="small" onClick={load}>Refresh</button>}
        </>
      )}
    </section>
  )
}
