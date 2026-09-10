const el = document.querySelector('.search'); const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
set.call(el, 'книгу'); el.dispatchEvent(new Event('input', { bubbles: true }))
await new Promise(r => setTimeout(r, 500))
