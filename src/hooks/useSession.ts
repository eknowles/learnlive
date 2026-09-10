import { useCallback, useEffect, useState } from 'react'
import { api } from '../lib/api'
import { defaultSources, modelKey } from '../lib/config'
import { asAppError, type AppError } from '../lib/errors'
import { loadConfig, onConfigChanged, saveConfig } from '../lib/prefs'
import type { AudioDevice, CalendarEvent, Language, Levels, MeetingSummary, ModelProgress, ModelStatus, SessionConfig } from '../lib/types'

export type Phase = 'setup' | 'downloading' | 'starting' | 'live'

/** Everything about the current session: config, devices, models, start/stop, live levels.
 *  Config is persisted and mirrored to the Settings window (see lib/prefs.ts). */
export function useSession(onStarted?: () => void, onStopped?: () => void) {
  const [cfg, setCfgState] = useState<SessionConfig>(loadConfig)
  const [devices, setDevices] = useState<AudioDevice[]>([])
  const [languages, setLanguages] = useState<Language[]>([])
  const [models, setModels] = useState<ModelStatus[]>([])
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({})
  const [phase, setPhase] = useState<Phase>('setup')
  const [levels, setLevels] = useState<Levels | null>(null)
  const [error, setError] = useState<AppError | null>(null)
  const [meeting, setMeeting] = useState<MeetingSummary | null>(null)
  const [pendingEvent, setPendingEvent] = useState<CalendarEvent | null>(null)

  /** Local edits persist + broadcast; edits arriving from the other window only update state. */
  const setCfg = useCallback((update: SessionConfig | ((c: SessionConfig) => SessionConfig)) => {
    setCfgState(prev => {
      const next = typeof update === 'function' ? update(prev) : update
      saveConfig(next)
      return next
    })
  }, [])

  useEffect(() => {
    api
      .devices()
      .then(ds => {
        setDevices(ds)
        // Keep remembered sources that still exist; otherwise pick sensible defaults.
        setCfgState(c => {
          const kept = c.sources.filter(s => ds.some(d => d.id === s.device_id))
          return { ...c, sources: kept.length ? kept : defaultSources(ds) }
        })
      })
      .catch(e => setError(asAppError(e)))
    api.languages().then(setLanguages)
    const subs = Promise.all([
      api.onLevels(setLevels),
      api.onModelProgress(p => setProgress(prev => ({ ...prev, [p.model]: p }))),
      api.onError(m => setError({ kind: 'internal', message: m })),
      onConfigChanged(setCfgState),
    ])
    return () => {
      subs.then(fns => fns.forEach(f => f()))
    }
  }, [])

  const key = modelKey(cfg)
  useEffect(() => {
    api
      .modelStatus(cfg)
      .then(setModels)
      .catch(() => {})
  }, [key])

  useEffect(() => {
    api.setListening(phase === 'live').catch(() => {})
  }, [phase])

  const start = async () => {
    if (phase !== 'setup') return
    setError(null)
    try {
      if (models.some(m => !m.present)) {
        setPhase('downloading')
        await api.prepareModels(cfg)
      }
      setPhase('starting')
      const id = await api.start(cfg, pendingEvent)
      setMeeting((await api.listMeetings(5)).find(m => m.id === id) ?? null)
      onStarted?.()
      setPhase('live')
    } catch (e) {
      setError(asAppError(e))
      setPhase('setup')
    }
  }
  const stop = async () => {
    if (phase !== 'live') return
    await api.stop()
    setPhase('setup')
    setPendingEvent(null)
    onStopped?.()
  }

  return {
    cfg,
    setCfg,
    devices,
    languages,
    models,
    progress,
    phase,
    levels,
    error,
    dismissError: () => setError(null),
    meeting,
    setMeeting,
    pendingEvent,
    setPendingEvent,
    start,
    stop,
  }
}
