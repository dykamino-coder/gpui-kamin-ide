// The application's own UI. Everything workspace-related goes through
// @kaminide/bridge-browser-client, served by the example at /client/index.js.
import { createBridgeClient } from '/client/index.js'

const $ = id => document.getElementById(id)
const client = createBridgeClient()
let tab = null
const lines = new Map()

function render() {
  $('transcript').textContent = [...lines.values()].join('\n\n')
}
function text(entry) {
  const content = entry?.message?.content
  if (typeof content === 'string') return content
  if (Array.isArray(content)) return content.map(part => part.text ?? '').filter(Boolean).join('\n')
  return ''
}

client.onConnectionState(state => { $('state').textContent = `Workspace: ${state}` })
client.onJsonlEntries((id, entries) => {
  if (id !== tab) return
  for (const entry of entries) if (entry.uuid && text(entry)) lines.set(entry.uuid, `${entry.type}: ${text(entry)}`)
  render()
})
client.onStreaming((id, entry) => {
  if (id !== tab) return
  lines.set('streaming', `assistant: ${text(entry)}`)
  render()
})
client.preview.onState(state => { $('preview').textContent = JSON.stringify(state, null, 2) })

async function open() {
  await client.connect()
  const snapshot = await client.sync()
  tab = snapshot.active ?? await client.createTab()
  if (!snapshot.active) client.switchTab(tab)
  $('prompt').hidden = false
}

$('sign-in').addEventListener('submit', async event => {
  event.preventDefault()
  // 1. The site authenticates the user and returns a one-time assertion.
  const site = await fetch('/site/login', { method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ user: $('user').value, password: $('password').value }) })
  $('password').value = ''
  if (!site.ok) { $('state').textContent = 'Sign-in failed.'; return }
  const { assertion } = await site.json()
  // 2. The workspace host turns the assertion into its session cookie.
  const login = await fetch('/auth/login', { method: 'POST', credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ token: assertion }) })
  if (!login.ok) { $('state').textContent = 'Workspace sign-in failed.'; return }
  $('sign-in').hidden = true
  await open().catch(error => { $('state').textContent = error.message })
})

$('prompt').addEventListener('submit', event => {
  event.preventDefault()
  if (!tab || !$('text').value.trim()) return
  client.submitPrompt(tab, $('text').value)
  $('text').value = ''
})
$('stop').addEventListener('click', () => { if (tab) client.interrupt(tab) })

// Resume an existing cookie session after a reload.
fetch('/auth/session', { credentials: 'same-origin' }).then(response => response.json()).then(session => {
  if (session.authenticated) { $('sign-in').hidden = true; return open() }
}).catch(() => {})
