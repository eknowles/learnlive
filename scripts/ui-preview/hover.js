// Live transcript with a word popover open, so the teaching affordance is visible.
document.querySelector('.btn.toolbar-item.primary').click()
await new Promise(r => setTimeout(r, 400))
for (const s of window.__mock.segments) { window.__mock.emit('segment', s); await new Promise(r => setTimeout(r, 30)) }
await new Promise(r => setTimeout(r, 250))
const words = document.querySelectorAll('.turn .study .w')
const target = words[3] ?? words[0]
target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }))
await new Promise(r => setTimeout(r, 400))
