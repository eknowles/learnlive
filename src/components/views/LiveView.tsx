import type { Phase } from '../../hooks/useSession'
import type { ModelProgress, ModelStatus, Segment } from '../../lib/types'
import Icon from '../ui/Icon'
import Transcript from '../Transcript'

interface Props {
  phase: Phase
  segments: Segment[]
  learning: string
  speechActive: boolean
  models: ModelStatus[]
  progress: Record<string, ModelProgress>
  canStart: boolean
}

const mb = (n: number) => (n / 1_048_576).toFixed(0)

/** The live surface: an empty state until the first sentence, then the transcript. */
export default function LiveView({ phase, segments, learning, speechActive, models, progress, canStart }: Props) {
  if (segments.length > 0) return <Transcript segments={segments} learning={learning} live={phase === 'live'} speechActive={speechActive} />

  const missing = models.filter(m => !m.present)
  const missingMb = missing.reduce((a, m) => a + m.approx_mb, 0)

  if (phase === 'downloading') {
    return (
      <div className="empty">
        <Icon name="arrow.down" size={40} />
        <h2>Downloading models</h2>
        <p>Once only. They live in the app’s data folder.</p>
        <ul className="downloads">
          {missing.map(m => {
            const p = progress[m.id]
            const pct = p?.total ? Math.min(100, (p.bytes / p.total) * 100) : p?.done ? 100 : 0
            return (
              <li key={m.id}>
                <span>{m.id}</span>
                <span className="secondary">{p?.done ? 'Done' : p ? `${mb(p.bytes)} MB` : 'Waiting…'}</span>
                <progress value={pct} max={100} />
              </li>
            )
          })}
        </ul>
      </div>
    )
  }
  if (phase === 'starting') {
    return (
      <div className="empty">
        <Icon name="waveform" size={40} />
        <h2>Loading models</h2>
        <p>A few seconds the first time.</p>
      </div>
    )
  }
  if (phase === 'live') {
    return (
      <div className="empty">
        <Icon name="waveform" size={40} />
        <h2>{speechActive ? 'Listening…' : 'Waiting for someone to speak'}</h2>
        <p>Sentences appear here as they’re said, then settle as more arrives.</p>
      </div>
    )
  }
  return (
    <div className="empty">
      <Icon name="waveform" size={40} />
      <h2>Ready when your call is</h2>
      <p>
        {canStart ? (
          <>
            Join the call, then press <b>Start Listening</b> or <kbd>⌘R</kbd>.
          </>
        ) : (
          'Choose a call audio source or microphone in the inspector to begin.'
        )}
      </p>
      {missing.length > 0 && canStart && (
        <p className="caption">The first start downloads {missingMb} MB of models. Everything runs on this Mac; nothing is uploaded.</p>
      )}
    </div>
  )
}
