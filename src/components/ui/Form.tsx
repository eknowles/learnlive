import type { ReactNode } from 'react'

/** A titled, inset group of rows — the System Settings idiom. */
export function Group({ title, footer, children }: { title?: string; footer?: ReactNode; children: ReactNode }) {
  return (
    <section className="group">
      {title && <h3 className="group-title">{title}</h3>}
      <div className="group-body">{children}</div>
      {footer && <p className="group-footer">{footer}</p>}
    </section>
  )
}

interface RowProps {
  label: ReactNode
  caption?: ReactNode
  /** Caption is a warning (e.g. "no loopback device"). */
  warn?: boolean
  /** Control below the label instead of beside it (long popups). */
  stack?: boolean
  children?: ReactNode
}

export function Row({ label, caption, warn, stack, children }: RowProps) {
  return (
    <label className={`row ${stack ? 'stack' : ''}`}>
      <span className="row-text">
        <span className="row-label">{label}</span>
        {caption && <span className={`row-caption ${warn ? 'warn' : ''}`}>{caption}</span>}
      </span>
      {children && <span className="row-control">{children}</span>}
    </label>
  )
}
