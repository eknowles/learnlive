/** Shape of errors thrown by every Tauri command (see src-tauri/src/error.rs). */
export interface AppError {
  kind: 'invalid' | 'permission' | 'models' | 'audio' | 'internal'
  message: string
}

export const asAppError = (e: unknown): AppError => {
  if (typeof e === 'object' && e && 'kind' in e && 'message' in e) return e as AppError
  return { kind: 'internal', message: String(e) }
}

/** What to tell the person, in the interface's voice. */
export const describe = (e: AppError): string => {
  switch (e.kind) {
    case 'permission':
      return `${e.message} Check System Settings → Privacy & Security.`
    case 'audio':
      return `${e.message} Pick a different device or check that BlackHole is installed.`
    case 'models':
      return `${e.message} Try Start again to re-download.`
    default:
      return e.message
  }
}
