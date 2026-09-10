import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import Settings from './windows/Settings'
import './styles/tokens.css'
import './styles/base.css'
import './styles/controls.css'
import './styles/shell.css'
import './styles/transcript.css'
import './styles/settings.css'

// One bundle, two windows: the Rust shell opens `index.html#settings` for ⌘, (see native.rs).
const isSettings = location.hash === '#settings'

// The webview's own context menu is a browser artefact. Keep it only where macOS would show
// one anyway (text fields), and let components that build a real NSMenu handle their own.
document.addEventListener('contextmenu', e => {
  const t = e.target as Element
  if (!t.closest('input, textarea')) e.preventDefault()
})

ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode>{isSettings ? <Settings /> : <App />}</React.StrictMode>)
