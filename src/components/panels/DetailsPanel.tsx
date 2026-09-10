import { clock } from '../../lib/time'
import type { Language, MeetingSummary, Segment } from '../../lib/types'
import { duration } from '../shell/Sidebar'
import { Group, Row } from '../ui/Form'
import SpeakersPanel from './SpeakersPanel'

interface Props {
  meeting: MeetingSummary
  segments: Segment[]
  languages: Language[]
  onRename: (id: number, label: string) => void
  onMeeting: (m: MeetingSummary) => void
}

const longDate = (s: number) =>
  new Date(s * 1000).toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })

/** Inspector for a past meeting: facts, people, and the same speaker assignment as live. */
export default function DetailsPanel({ meeting, segments, languages, onRename, onMeeting }: Props) {
  const name = (code: string) => languages.find(l => l.code === code)?.name ?? code
  return (
    <>
      <Group title="Details">
        <Row label="Date" caption={`${longDate(meeting.started_at)} at ${clock(meeting.started_at * 1000).slice(0, 5)}`} />
        <Row label="Length" caption={`${duration(meeting)} · ${meeting.sentence_count} sentences`} />
        <Row label="Languages" caption={`${name(meeting.learning)} → ${name(meeting.native)}`} />
      </Group>
      {meeting.participants.length > 0 && (
        <Group title="Invited">
          {meeting.participants.map(p => (
            <Row key={p.id} label={p.name} caption={p.email ?? undefined} />
          ))}
        </Group>
      )}
      <SpeakersPanel segments={segments} meeting={meeting} onRename={onRename} onMeeting={onMeeting} />
    </>
  )
}
