import type { ReactNode, SVGProps } from 'react'

/** SF Symbols–style glyphs: 16pt grid, 1.5pt rounded strokes, so they sit next to system text
 *  the way the real symbols do. Add new ones here rather than importing an icon pack. */
export type IconName =
  | 'sidebar.left'
  | 'sidebar.right'
  | 'waveform'
  | 'stop'
  | 'magnifyingglass'
  | 'speaker'
  | 'calendar'
  | 'person'
  | 'mic'
  | 'gear'
  | 'trash'
  | 'xmark'
  | 'arrow.down'
  | 'warning'
  | 'clock'
  | 'checkmark'
  | 'text.bubble'
  | 'arrow.clockwise'

const glyphs: Record<IconName, ReactNode> = {
  'sidebar.left': (
    <>
      <rect x="1.75" y="3" width="12.5" height="10" rx="2.25" />
      <path d="M6 3v10" />
    </>
  ),
  'sidebar.right': (
    <>
      <rect x="1.75" y="3" width="12.5" height="10" rx="2.25" />
      <path d="M10 3v10" />
    </>
  ),
  waveform: <path d="M2 6.5v3M4.5 4v8M7 2v12M9.5 5v6M12 3v10M14.5 6.5v3" />,
  stop: <rect x="3.25" y="3.25" width="9.5" height="9.5" rx="2" fill="currentColor" stroke="none" />,
  magnifyingglass: (
    <>
      <circle cx="6.75" cy="6.75" r="4.25" />
      <path d="M10 10l4 4" />
    </>
  ),
  speaker: (
    <>
      <path d="M2.5 6h2.4l3.1-2.6v9.2L4.9 10H2.5z" strokeLinejoin="round" />
      <path d="M10.4 5.6a3.4 3.4 0 0 1 0 4.8M12.6 3.4a6.5 6.5 0 0 1 0 9.2" />
    </>
  ),
  calendar: (
    <>
      <rect x="2" y="3" width="12" height="11" rx="2" />
      <path d="M2 6.5h12M5 1.5v3M11 1.5v3" />
    </>
  ),
  person: (
    <>
      <circle cx="8" cy="5" r="2.75" />
      <path d="M2.75 14a5.25 5.25 0 0 1 10.5 0" />
    </>
  ),
  mic: (
    <>
      <rect x="5.5" y="1.5" width="5" height="8" rx="2.5" />
      <path d="M3 7.5a5 5 0 0 0 10 0M8 12.5v2M5.5 14.5h5" />
    </>
  ),
  gear: (
    <>
      <circle cx="8" cy="8" r="2.25" />
      <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" />
    </>
  ),
  trash: (
    <>
      <path d="M2.5 4h11M6 4V2.5h4V4M4 4l.7 9.5h6.6L12 4" strokeLinejoin="round" />
      <path d="M6.5 7v4M9.5 7v4" />
    </>
  ),
  xmark: <path d="M4 4l8 8M12 4l-8 8" />,
  'arrow.down': <path d="M8 2.5v11M3.5 9l4.5 4.5L12.5 9" strokeLinejoin="round" />,
  warning: (
    <>
      <path d="M8 2l6.5 11.5h-13z" strokeLinejoin="round" />
      <path d="M8 6.5v3.5M8 12.1v.15" />
    </>
  ),
  clock: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="M8 4.5V8l2.5 1.5" />
    </>
  ),
  checkmark: <path d="M3 8.5l3.2 3L13 4.5" strokeLinejoin="round" />,
  'text.bubble': (
    <>
      <path d="M2.5 3.5h11v7.5H7l-3 2.5v-2.5H2.5z" strokeLinejoin="round" />
      <path d="M5 6.5h6M5 8.5h4" />
    </>
  ),
  'arrow.clockwise': (
    <>
      <path d="M13 8a5 5 0 1 1-1.5-3.6" />
      <path d="M12.5 1.5v3.2h-3.2" strokeLinejoin="round" />
    </>
  ),
}

interface Props extends SVGProps<SVGSVGElement> {
  name: IconName
  size?: number
}

export default function Icon({ name, size = 16, ...rest }: Props) {
  return (
    <svg
      className="icon"
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      aria-hidden
      {...rest}
    >
      {glyphs[name]}
    </svg>
  )
}
