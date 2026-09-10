import type { ReactNode } from 'react'

export default function Inspector({ children }: { children: ReactNode }) {
  return (
    <aside className="inspector" aria-label="Inspector">
      <div className="inspector-inner">{children}</div>
    </aside>
  )
}
