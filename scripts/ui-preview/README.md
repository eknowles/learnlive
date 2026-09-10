# UI preview without the app

Renders the React UI in a bare `WKWebView` (the same engine Tauri uses, so AppKit system colours,
`ui-serif` and native controls look right) with the Tauri bridge mocked, and writes a PNG.
Useful for eyeballing a change without models, audio devices or screen-recording permission.

```sh
npx vite &                                   # dev server on :5173
just ui-preview                              # → out/ui-*.png for the main states
scripts/ui-preview/render http://localhost:5173/ out.png 1180 780 dark scripts/ui-preview/live.js
```

`mock.js` is injected before the page loads and fakes `window.__TAURI_INTERNALS__` with sample
devices, languages, meetings and segments. A scenario file is plain JS run after load (`window.__mock.emit`
fires a Tauri event). The sidebar's vibrancy is approximated with a flat tint in the composite.
The harness window is never key, so accent-tinted native controls (switches) render in their inactive grey.
