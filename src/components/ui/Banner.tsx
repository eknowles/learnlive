import { api } from '../../lib/api'
import { describe, privacyPane, type AppError } from '../../lib/errors'
import Button from './Button'
import Icon from './Icon'

/** Inline, dismissible problem report — errors in a running app are not modal. */
export default function Banner({ error, onDismiss }: { error: AppError; onDismiss: () => void }) {
  const pane = privacyPane(error)
  return (
    <div className="banner" role="alert">
      <Icon name="warning" />
      <p>{describe(error)}</p>
      {pane && <Button onClick={() => api.openPrivacySettings(pane)}>Open System Settings…</Button>}
      <Button variant="plain" icon="xmark" aria-label="Dismiss" onClick={onDismiss} />
    </div>
  )
}
