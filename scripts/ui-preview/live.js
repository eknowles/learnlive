document.querySelector('.btn.toolbar-item.primary').click()
await new Promise(r => setTimeout(r, 400))
for (const s of window.__mock.segments) { window.__mock.emit('segment', s); await new Promise(r => setTimeout(r, 30)) }
window.__mock.emit('levels', { per_source: [['bh', 0.25], ['mic', 0.04]], mix: 0.3, speech_active: true })
