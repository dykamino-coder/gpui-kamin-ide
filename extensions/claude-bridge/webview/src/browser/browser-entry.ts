import { installBrowserApi } from '../lib/webview-api'
import { createBrowserSocket, type ConnectionState } from './browser-socket'
import './browser.css'
import { installBrowserEffects } from './browser-effects'
import { installPreviewPanel } from './preview-panel'

// Authentication precedes importing the shared renderer: module initialization
// reads the API and starts version requests. No token enters renderer state.
const root = document.getElementById('root')!
const gate = document.getElementById('workspace-gate')!
const message = document.getElementById('workspace-status')!
const form = document.getElementById('workspace-login') as HTMLFormElement
const token = document.getElementById('workspace-token') as HTMLInputElement
const submit = form.querySelector('button')!
const logout = document.getElementById('workspace-logout') as HTMLButtonElement
let transport: ReturnType<typeof createBrowserSocket> | undefined
let mounting = false
let mounted = false
let renderer: typeof import('./browser-renderer') | undefined
let disposePreview: (() => void) | undefined

function mountWhenReady(): void {
  if (!online || !renderer) return
  if (!mounted) {
    renderer.mount(root)
    mounted = true
  }
  unlock()
}

function locked(text: string, login = false): void {
  root.setAttribute('inert', '')
  root.setAttribute('aria-hidden', 'true')
  gate.hidden = false
  message.textContent = text
  form.hidden = !login
}

async function session(): Promise<boolean> {
  const response = await fetch('/auth/session', { credentials: 'same-origin', cache: 'no-store' })
  if (!response.ok) throw new Error('Session unavailable')
  const data = await response.json() as { authenticated?: unknown }
  return data.authenticated === true
}

function connection(state: ConnectionState): void {
  if (state === 'connected') {
    if (!mounting) {
      mounting = true
      void import('./browser-renderer').then(module => {
        renderer = module
        mountWhenReady()
      }).catch(() => {
        transport?.dispose()
        transport = undefined
        locked('Workspace could not start. Reload to try again.')
      })
    } else mountWhenReady()
  } else if (state === 'authentication-required') {
    transport?.dispose()
    transport = undefined
    locked('Session ended. Sign in again.', true)
  } else if (state === 'closed') {
    locked('Workspace connection stopped. Reload to try again.')
  } else {
    locked(state === 'connecting' ? 'Connecting to workspace…' : 'Connection lost. Reconnecting; pending operations may already have run.')
    // A failed upgrade has no readable status code. Check the HTTP session so
    // expired credentials do not leave a perpetual reconnect screen.
    if (state === 'offline') void session().then(valid => {
      if (!valid && !online) {
        transport?.dispose()
        transport = undefined
        locked('Sign in to the workspace.', true)
      }
    }).catch(() => { /* transient host outage: socket backoff continues */ })
  }
}

let online = false
function unlock(): void {
  if (!online) return
  root.removeAttribute('inert')
  root.removeAttribute('aria-hidden')
  gate.hidden = true
  logout.hidden = false
}

function start(): void {
  const url = new URL('/bridge', location.href)
  url.protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  transport = createBrowserSocket({ url: url.href, fileViewIdentity: {
    get() { try { return sessionStorage.getItem('workspace-file-view') ?? undefined } catch { return undefined } },
    set(id) { try { sessionStorage.setItem('workspace-file-view', id) } catch { /* host adopts detached views */ } },
  }, onState(state) {
    online = state === 'connected'
    connection(state)
  } })
  installBrowserApi(transport)
  installBrowserEffects(transport)
  disposePreview = installPreviewPanel()
  transport.connect()
}

form.addEventListener('submit', event => {
  event.preventDefault()
  submit.disabled = true
  // Clear the field immediately. Keep the value only in this request body.
  const body = JSON.stringify({ token: token.value })
  token.value = ''
  void fetch('/auth/login', {
    method: 'POST', credentials: 'same-origin', cache: 'no-store',
    headers: { 'Content-Type': 'application/json' }, body,
  }).then(response => {
    if (!response.ok) {
      locked(response.status === 429 ? 'Too many attempts. Try again later.' : 'Sign-in failed.', true)
      return
    }
    // Fresh module state and document generation after reauthentication.
    location.reload()
  }).catch(() => locked('Workspace unavailable. Try again.', true))
    .finally(() => { submit.disabled = false })
})

logout.addEventListener('click', () => {
  logout.disabled = true
  void fetch('/auth/logout', { method: 'POST', credentials: 'same-origin' }).then(response => {
    if (!response.ok) throw new Error('Sign-out failed')
    transport?.dispose()
    location.reload()
  }).catch(() => {
    logout.disabled = false
    locked('Sign-out failed. Reload to try again.')
  })
})
window.addEventListener('pagehide', () => { disposePreview?.(); transport?.dispose() })
locked('Checking workspace session…')
void session().then(valid => {
  if (valid) start()
  else locked('Sign in to the workspace.', true)
}).catch(() => locked('Workspace unavailable. Reload to try again.'))
