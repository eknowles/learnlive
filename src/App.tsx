import { useState } from 'react'
import Setup from './components/Setup'
import Mixer from './components/Mixer'
import Speakers from './components/Speakers'
import Meeting from './components/Meeting'
import History from './components/History'
import Transcript from './components/Transcript'
import { useSession } from './hooks/useSession'
import { useTranscript } from './hooks/useTranscript'
import './styles.css'

export default function App() {
  const [view, setView] = useState<'live' | 'history'>('live')
  const transcript = useTranscript()
  const s = useSession(transcript.reset)

  return (
    <div className="shell">
      <aside className="rail">
        <h1 className="brand">LearnLive</h1>
        <nav className="views" aria-label="View">
          <button aria-pressed={view === 'live'} onClick={() => setView('live')}>
            Live
          </button>
          <button aria-pressed={view === 'history'} onClick={() => setView('history')}>
            History
          </button>
        </nav>
        <Setup
          cfg={s.cfg}
          setCfg={s.setCfg}
          devices={s.devices}
          languages={s.languages}
          models={s.models}
          progress={s.progress}
          phase={s.phase}
          onStart={s.start}
          onStop={s.stop}
        />
        <Meeting phase={s.phase} meeting={s.meeting} pending={s.pendingEvent} onPending={s.setPendingEvent} onLinked={s.setMeeting} />
        {s.phase === 'live' && <Mixer cfg={s.cfg} setCfg={s.setCfg} devices={s.devices} levels={s.levels} />}
        <Speakers segments={transcript.segments} meeting={s.meeting} onRename={transcript.rename} onMeeting={s.setMeeting} />
        {s.error && (
          <p className="error" role="alert">
            {s.error}
          </p>
        )}
      </aside>
      <main className="stage">
        {view === 'history' ? (
          <History />
        ) : (
          <Transcript
            segments={transcript.segments}
            learning={s.cfg.learning}
            live={s.phase === 'live'}
            speechActive={s.levels?.speech_active ?? false}
          />
        )}
      </main>
    </div>
  )
}
