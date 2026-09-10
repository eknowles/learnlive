import type { OpenMeeting } from '../../hooks/useHistory'
import { clock } from '../../lib/time'
import { duration } from '../shell/Sidebar'
import Transcript from '../Transcript'

const day = (s: number) => new Date(s * 1000).toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })

/** A past meeting: the same reading surface, with a heading that carries the facts. */
export default function MeetingView({ meeting: m, segments }: OpenMeeting) {
  return (
    <article className="meeting">
      <header className="meeting-head selectable">
        <h2>{m.title}</h2>
        <p>
          {day(m.started_at)} · {clock(m.started_at * 1000).slice(0, 5)} · {duration(m)} · {segments.length} sentences
          {m.participants.length > 0 && <> · with {m.participants.map(p => p.name).join(', ')}</>}
        </p>
      </header>
      <Transcript segments={segments} learning={m.learning} live={false} speechActive={false} />
    </article>
  )
}
