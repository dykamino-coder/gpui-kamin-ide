// Only the panel chrome lives here. The host owns mount/props and dev-server
// availability and pushes `workspace:preview-state` over /bridge; the shared
// preview view (browser-client) follows it and renders the remote.
import type { HostPort } from '../../../browser-client/src/socket'
import type { PreviewState } from '../../../browser-client/src/types'
import { createPreviewView } from '../../../browser-client/src/preview/view'
import type { RenderRemote } from '../../../browser-client/src/preview/renderer'

/** Host event frames from the installed socket; this module must not import the
 * shared transport, which binds to the API only after authentication. */
function previewStates(port: Required<Pick<HostPort, 'onHostMessage'>>) {
  return (listener: (state: PreviewState) => void) => port.onHostMessage(frame => {
    const event = frame as { kind?: unknown; channel?: unknown; args?: unknown[] } | null
    if (event?.kind === 'event' && event.channel === 'workspace:preview-state' && event.args?.[0] && typeof event.args[0] === 'object') {
      listener(event.args[0] as PreviewState)
    }
  })
}

export function installPreviewPanel(port: Required<Pick<HostPort, 'onHostMessage'>>): () => void {
  const chat = document.getElementById('root')!
  const layout = document.createElement('div')
  layout.className = 'workspace-layout'
  chat.before(layout)
  layout.append(chat)
  const separator = document.createElement('div')
  separator.className = 'preview-resize'
  separator.setAttribute('role', 'separator')
  separator.setAttribute('aria-label', 'Resize Preview')
  separator.setAttribute('aria-orientation', 'vertical')
  separator.tabIndex = 0
  const panel = document.createElement('section')
  panel.className = 'workspace-preview'
  panel.setAttribute('aria-label', 'Preview')
  panel.hidden = separator.hidden = true
  const header = document.createElement('header')
  const title = document.createElement('strong')
  const retry = document.createElement('button')
  retry.textContent = 'Reload preview'
  const close = document.createElement('button')
  close.textContent = 'Close preview'
  header.append(title, retry, close)
  const status = document.createElement('div')
  status.setAttribute('role', 'status')
  const content = document.createElement('div')
  content.className = 'preview-content'
  panel.append(header, status, content)
  layout.append(separator, panel)
  // React and federation load only when a remote is first rendered.
  const render: RenderRemote = async (...args) => (await import('./preview-renderer')).renderRemote(...args)
  const view = createPreviewView({ states: previewStates(port), render, element: content })
  const fail = (error: unknown) => { status.textContent = `Preview: ${error instanceof Error ? error.message : 'Remote failed'}` }
  view.onChange(() => {
    const descriptor = view.descriptor
    panel.hidden = separator.hidden = !descriptor
    if (descriptor) title.textContent = `Preview · ${descriptor.name}`
    status.textContent = view.status === 'mounting' ? 'Loading preview…'
      : view.status === 'unavailable' ? 'Preview dev server is unavailable. The agent can restart it.'
        : view.status === 'error' ? `Preview: ${view.error?.message || 'Remote failed'}` : ''
  })
  retry.onclick = () => view.reload()
  close.onclick = () => { void fetch('/preview/unmount', { method: 'POST', credentials: 'same-origin' }).catch(fail) }
  const width = (value: number) => { panel.style.width = `${Math.max(240, Math.min(innerWidth - 1106, value))}px` }
  separator.onpointerdown = event => { separator.setPointerCapture(event.pointerId) }
  separator.onpointermove = event => { if (separator.hasPointerCapture(event.pointerId)) width(innerWidth - event.clientX) }
  separator.onkeydown = event => {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); width(panel.offsetWidth + (event.key === 'ArrowLeft' ? 32 : -32)) }
  }
  return () => view.dispose()
}
