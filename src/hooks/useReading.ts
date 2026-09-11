import { useCallback, useEffect, useRef, useState } from 'react'
import { loadReading, onReadingChanged, saveReading } from '../lib/prefs'
import { applyReading, DEFAULT_READING, nextMode, type ReadingPrefs } from '../lib/reading'

/** Reading preferences, applied to the document and kept in step with the Settings window. */
export function useReading() {
  const [reading, setReading] = useState<ReadingPrefs>(loadReading)
  // The setters are stable so they can be handed to menus and memoised children; the latest
  // value comes from here rather than from the closure they were created in.
  const latest = useRef(reading)
  latest.current = reading

  useEffect(() => {
    applyReading(reading)
  }, [reading])

  useEffect(() => {
    const p = onReadingChanged(setReading)
    return () => {
      p.then(f => f())
    }
  }, [])

  const update = useCallback((patch: Partial<ReadingPrefs>) => {
    const next = { ...latest.current, ...patch }
    latest.current = next
    setReading(next)
    saveReading(next)
  }, [])

  const cycle = useCallback(() => update({ mode: nextMode(latest.current.mode) }), [update])
  const reset = useCallback(() => update(DEFAULT_READING), [update])

  return { reading, update, cycle, reset }
}
