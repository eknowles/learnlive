import type { InputHTMLAttributes } from 'react'

/** WebKit renders `<input type=checkbox switch>` as a real NSSwitch (follows the accent colour). */
export default function Switch(props: Omit<InputHTMLAttributes<HTMLInputElement>, 'type'>) {
  return <input type="checkbox" className="switch" {...{ switch: '' }} {...props} />
}
