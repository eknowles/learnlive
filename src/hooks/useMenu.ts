import { useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'

/** Menu-bar items the Rust shell forwards (`src-tauri/src/native.rs`), keyed by item id. */
export function useMenu(handlers: Record<string, () => void>) {
  const ref = useRef(handlers)
  ref.current = handlers
  useEffect(() => {
    const p = listen<string>('menu', e => ref.current[e.payload]?.())
    return () => {
      p.then(f => f())
    }
  }, [])
}
