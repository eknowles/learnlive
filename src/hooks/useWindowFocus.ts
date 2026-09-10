import { useEffect } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'

/** Mirrors window activation onto <html data-inactive>, so chrome can dim like AppKit does. */
export function useWindowFocus() {
  useEffect(() => {
    const root = document.documentElement
    const apply = (focused: boolean) => root.toggleAttribute('data-inactive', !focused)
    apply(document.hasFocus())
    const p = getCurrentWindow().onFocusChanged(e => apply(e.payload))
    return () => {
      p.then(f => f())
    }
  }, [])
}
