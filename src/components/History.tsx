import { useEffect, useState } from 'react'
import { api } from '../lib/api'
import { clock } from '../lib/time'
import type { MeetingSummary, SearchHit, Segment } from '../lib/types'
import Transcript from './Transcript'

const day = (s: number) => new Date(s * 1000).toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' })
const mins = (m: MeetingSummary) => (m.ended_at ? `${Math.max(1, Math.round((m.ended_at - m.started_at) / 60))} min` : 'in progress')

/** Past meetings: list, open one, or search every sentence you've ever heard. */
export default function History() {
  const [meetings, setMeetings] = useState<MeetingSummary[]>([])
  const [open, setOpen] = useState<{ m: MeetingSummary; segments: Segment[] } | null>(null)
  const [q, setQ] = useState('')
  const [hits, setHits] = useState<SearchHit[]>([])
  const [remember, setRemember] = useState(false)

  useEffect(() => {
    api.listMeetings().then(setMeetings)
    api.getRememberVoices().then(setRemember)
  }, [])
  useEffect(() => {
    if (!q.trim()) {
      setHits([])
      return
    }
    const t = setTimeout(
      () =>
        api
          .search(q)
          .then(setHits)
          .catch(() => setHits([])),
      200,
    )
    return () => clearTimeout(t)
  }, [q])

  const openMeeting = async (id: number) => {
    const r = await api.getMeeting(id)
    if (r) setOpen({ m: r[0], segments: r[1] })
  }
  const remove = async (id: number) => {
    if (!confirm('Delete this meeting, its transcript and audio clips?')) return
    await api.deleteMeeting(id)
    setOpen(null)
    setMeetings(await api.listMeetings())
  }
  const toggleRemember = async (on: boolean) => {
    if (!on && !confirm('Forget every stored voice? People will be labelled "Speaker 1, 2…" again until you assign them.')) return
    await api.setRememberVoices(on)
    setRemember(on)
  }

  if (open) {
    const { m, segments } = open
    return (
      <div className="history">
        <div className="history-head">
          <button onClick={() => setOpen(null)}>All meetings</button>
          <h1 className="brand">{m.title}</h1>
          <span className="quiet">
            {day(m.started_at)} · {clock(m.started_at * 1000).slice(0, 5)} · {mins(m)} · {segments.length} sentences
          </span>
          <button className="danger" onClick={() => remove(m.id)}>
            Delete
          </button>
        </div>
        {m.participants.length > 0 && <p className="quiet">With {m.participants.map(p => p.name).join(', ')}</p>}
        <Transcript segments={segments} learning={m.learning} live={false} speechActive={false} />
      </div>
    )
  }

  return (
    <div className="history">
      <div className="history-head">
        <h1 className="brand">Past meetings</h1>
        <input
          className="search"
          placeholder="Search everything that was said…  (e.g. книгу, book, instrument*)"
          value={q}
          onChange={e => setQ(e.target.value)}
        />
      </div>

      {q.trim() ? (
        <ul className="hits">
          {hits.length === 0 && <li className="quiet">No matches.</li>}
          {hits.map(h => (
            <li key={h.segment.id}>
              <button className="linklike" onClick={() => openMeeting(h.meeting_id)}>
                {h.meeting_title}
              </button>
              <span className="quiet">
                {' '}
                · {day(h.started_at)} · {h.segment.speaker.label}
              </span>
              <p className="study small">{h.segment.target_lang === 'en' ? h.segment.source_text : h.segment.target_text}</p>
              <p className="gloss">{h.segment.target_lang === 'en' ? h.segment.target_text : h.segment.source_text}</p>
            </li>
          ))}
        </ul>
      ) : (
        <ul className="meetings">
          {meetings.length === 0 && <li className="quiet">No meetings yet. Start listening and this fills in.</li>}
          {meetings.map(m => (
            <li key={m.id}>
              <button className="linklike title" onClick={() => openMeeting(m.id)}>
                {m.title}
              </button>
              <span className="quiet">
                {day(m.started_at)} · {clock(m.started_at * 1000).slice(0, 5)} · {mins(m)} · {m.sentence_count} sentences
                {m.participants.length ? ` · ${m.participants.map(p => p.name).join(', ')}` : ''}
              </span>
            </li>
          ))}
        </ul>
      )}

      <label className="row remember">
        <input type="checkbox" checked={remember} onChange={e => toggleRemember(e.target.checked)} />
        Remember the voices of people I've named, so they're recognised next time
        <span className="quiet small">Stored only on this Mac. Turning this off deletes them.</span>
      </label>
    </div>
  )
}
