import type { ButtonHTMLAttributes } from 'react'
import Icon, { type IconName } from './Icon'

type Variant = 'push' | 'primary' | 'destructive' | 'plain' | 'toolbar' | 'toolbar-primary' | 'toolbar-stop'

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant
  icon?: IconName
  /** Toggle state for toolbar toggles (rendered as aria-pressed). */
  pressed?: boolean
}

/** One button component, styled per macOS control role rather than per screen. */
export default function Button({ variant = 'push', icon, pressed, className = '', children, ...rest }: Props) {
  // Toolbar variants become `toolbar-item [primary|stop]`, never `toolbar`, which is the container's class.
  const role = variant.startsWith('toolbar') ? `toolbar-item ${variant.slice('toolbar-'.length)}` : variant
  const classes = ['btn', role.trim(), icon && !children ? 'icon-only' : '', className].filter(Boolean).join(' ')
  return (
    <button type="button" className={classes} aria-pressed={pressed} {...rest}>
      {icon && <Icon name={icon} />}
      {children}
    </button>
  )
}
