import { useState } from 'react'
import { api } from '../../lib/api'
import { speakerColor } from '../../lib/pos'
import type { MeetingSummary, Segment } from '../../lib/types'
import { Group } from '../ui/Form'

interface Props {
  segments: Segment[]
  meeting: MeetingSummary | null
  onRename: (id: number, label: string) => void
  onMeeting: (m: MeetingSummary) => void
}

/** Who's talking — with attendees from the linked invite as one-click assignments. */
export default function SpeakersPanel({ segments, meeting, onRename, onMeeting }: Props) {
  const [editing, setEditing] = useState<number | null>(null)
  const seen = new Map<number, { label: string; count: number }>()
  for (const s of segments) {
    const e = seen.get(s.speaker.id)
    seen.set(s.speaker.id, { label: s.speaker.label, count: (e?.count ?? 0) + 1 })
  }
  if (seen.size === 0) return null
  const attendees = meeting?.participants ?? []
  const assignedTo = (id: number) => attendees.find(p => p.speaker_id === id)

  const assign = async (speakerId: number, value: string) => {
    if (!meeting) return
    if (value === '__other') {
      setEditing(speakerId)
      return
    }
    const p = attendees.find(a => String(a.id) === value)
    if (!p) return
    onRename(speakerId, p.name)
    onMeeting(await api.assignSpeaker(meeting.id, speakerId, p.name, p.email))
  }
  const rename = async (speakerId: number, name: string) => {
    onRename(speakerId, name)
    setEditing(null)
    if (meeting) onMeeting(await api.assignSpeaker(meeting.id, speakerId, name, null))
    else api.renameSpeaker(speakerId, name)
  }

  return (
    <Group title="Who’s Talking" footer={attendees.length ? 'Pick a name to label every line from that voice.' : 'Click a name to rename.'}>
      {[...seen.entries()]
        .sort((a, b) => a[0] - b[0])
        .map(([id, { label, count }]) => {
          const a = assignedTo(id)
          return (
            <div key={id} className="row speaker">
              <span className="swatch" style={{ background: speakerColor(id) }} />
              {editing === id ? (
                <input
                  className="speaker-name"
                  autoFocus
                  defaultValue={label}
                  onBlur={e => rename(id, e.target.value || label)}
                  onKeyDown={e => {
                    if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
                    if (e.key === 'Escape') setEditing(null)
                  }}
                />
              ) : id === 0 || attendees.length === 0 ? (
                <button type="button" className="speaker-name link-quiet" onClick={() => setEditing(id)} title="Rename">
                  {label}
                </button>
              ) : (
                <select
                  className="speaker-name"
                  value={a ? String(a.id) : ''}
                  onChange={e => assign(id, e.target.value)}
                  aria-label={`Who is ${label}?`}
                >
                  <option value="" disabled>
                    {label} — who is this?
                  </option>
                  {attendees.map(p => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                      {p.has_voiceprint ? ' ✓' : ''}
                    </option>
                  ))}
                  <option value="__other">Someone else…</option>
                </select>
              )}
              <span className="count">{count}</span>
            </div>
          )
        })}
    </Group>
  )
}
