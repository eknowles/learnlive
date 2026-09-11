// The same log with the language switch set to "Learning" only.
document.querySelector('.btn.toolbar-item.primary').click()
await new Promise(r => setTimeout(r, 400))
for (const s of window.__mock.segments) { window.__mock.emit('segment', s); await new Promise(r => setTimeout(r, 30)) }
await new Promise(r => setTimeout(r, 200))
;[...document.querySelectorAll('.segmented button')].find(b => b.textContent.trim() === 'Learning').click()
await new Promise(r => setTimeout(r, 300))
