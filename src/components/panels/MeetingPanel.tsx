import { useEffect, useState } from 'react'
import { api } from '../../lib/api'
import { asAppError, describe } from '../../lib/errors'
import { clock } from '../../lib/time'
import type { CalendarEvent, MeetingSummary } from '../../lib/types'
import Button from '../ui/Button'
import { Group, Row } from '../ui/Form'

interface Props {
  live: boolean
  meeting: MeetingSummary | null
  pending: CalendarEvent | null // chosen before Start
  onPending: (e: CalendarEvent | null) => void
  onLinked: (m: MeetingSummary) => void
}

/** Which calendar event is this session? Pick before Start; shown as linked during. */
export default function MeetingPanel({ live, meeting, pending, onPending, onLinked }: Props) {
  const [events, setEvents] = useState<CalendarEvent[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = () =>
    api
      .calendarNearNow()
      .then(evs => {
        setEvents(evs)
        setError(null)
        if (!pending && evs.length === 1 && !meeting) onPending(evs[0])
      })
      .catch(e => setError(describe(asAppError(e))))

  useEffect(() => {
    load()
  }, [])

  const choose = async (e: CalendarEvent | null) => {
    if (meeting && e) onLinked(await api.linkMeeting(meeting.id, e))
    else onPending(e)
  }

  const linked = meeting?.calendar_event_id ? meeting : null
  const caption = error
    ? error
    : linked
      ? `${clock(linked.started_at * 1000).slice(0, 5)} · ${linked.participants.length} invited`
      : events === null
        ? 'Looking at your calendar…'
        : events.length === 0
          ? 'Nothing on your calendar right now. The session is saved untitled; you can attach it later.'
          : undefined

  return (
    <Group title="Meeting">
      {linked ? (
        <Row label={linked.title} caption={caption} />
      ) : (
        <Row label="Calendar event" caption={caption} stack={!!events?.length}>
          {events && events.length > 0 && (
            <select value={pending?.id ?? ''} onChange={e => choose(events.find(x => x.id === e.target.value) ?? null)}>
              <option value="">Not a calendar meeting</option>
              {events.map(e => (
                <option key={e.id} value={e.id}>
                  {e.title} · {clock(e.start * 1000).slice(0, 5)}
                </option>
              ))}
            </select>
          )}
          {(live || error) && <Button variant="plain" icon="arrow.clockwise" aria-label="Refresh calendar" onClick={load} />}
        </Row>
      )}
    </Group>
  )
}
