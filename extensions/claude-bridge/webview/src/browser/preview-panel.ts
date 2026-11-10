// Only projection and module rendering live here. The host owns mount/props and
// dev-server availability, including while no browser page exists.
import type { RemoteDescriptor, Mounted } from './preview-renderer'

export function installPreviewPanel(): () => void {
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
  let disposed = false, epoch = 0, key = '', props = '', fingerprint = '', changedAt = 0
  let mounted: Mounted | undefined
  let descriptor: RemoteDescriptor | null = null
  function clear() {
    ++epoch
    try { mounted?.unmount() } catch { /* a remote cleanup cannot stop chat */ }
    mounted = undefined
    content.replaceChildren()
  }
  function fail(error: unknown) { status.textContent = `Preview: ${error instanceof Error ? error.message : 'Remote failed'}` }
  async function mount(value: RemoteDescriptor, build: string) {
    clear()
    const generation = epoch
    status.textContent = 'Loading preview…'
    const element = document.createElement('div')
    content.append(element)
    try {
      const { renderRemote } = await import('./preview-renderer')
      if (disposed || generation !== epoch) return
      const result = await renderRemote(element, value, build, fail,
        () => {},
        () => { if (generation === epoch && descriptor) void mount(descriptor, fingerprint) })
      if (disposed || generation !== epoch) { result.unmount(); return }
      mounted = result
      status.textContent = ''
    } catch (error) { if (generation === epoch) { element.replaceChildren(); fail(error) } }
  }
  retry.onclick = () => { if (descriptor) void mount(descriptor, fingerprint) }
  close.onclick = () => { void fetch('/preview/unmount', { method: 'POST', credentials: 'same-origin' }).catch(fail) }
  const width = (value: number) => { panel.style.width = `${Math.max(240, Math.min(innerWidth - 1106, value))}px` }
  separator.onpointerdown = event => { separator.setPointerCapture(event.pointerId) }
  separator.onpointermove = event => { if (separator.hasPointerCapture(event.pointerId)) width(innerWidth - event.clientX) }
  separator.onkeydown = event => {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); width(panel.offsetWidth + (event.key === 'ArrowLeft' ? 32 : -32)) }
  }
  let timer: ReturnType<typeof setTimeout>
  async function poll() {
    try {
      const response = await fetch('/preview/state', { credentials: 'same-origin', cache: 'no-store' })
      if (response.status === 404) return
      if (!response.ok) throw new Error('Workspace session unavailable')
      const state = await response.json() as { descriptor: RemoteDescriptor | null; build: string | null; available: boolean }
      if (disposed) return
      descriptor = state.descriptor
      panel.hidden = separator.hidden = !descriptor
      if (!descriptor) { if (key) clear(); key = props = fingerprint = ''; return }
      title.textContent = `Preview · ${descriptor.name}`
      if (!state.available || !state.build) { status.textContent = 'Preview dev server is unavailable. The agent can restart it.'; return }
      const nextKey = `${descriptor.name}:${descriptor.revision}`
      const nextProps = JSON.stringify(descriptor.props)
      if (nextKey !== key) {
        key = nextKey; props = nextProps; fingerprint = state.build; changedAt = 0
        void mount(descriptor, state.build)
      } else if (props !== nextProps) {
        props = nextProps
        if (mounted?.update) { try { mounted.update(descriptor.props) } catch (error) { fail(error) } }
        else void mount(descriptor, state.build)
      }
      // A native HMR runtime owns its updates and reports invalidation directly.
      // Only servers without HMR use the stable-entry rebuild fallback.
      if (fingerprint !== state.build) {
        if (!changedAt) changedAt = Date.now()
        if (mounted?.hot) { fingerprint = state.build; changedAt = 0 }
        else if (Date.now() - changedAt > 5000) { fingerprint = state.build; changedAt = 0; void mount(descriptor, state.build) }
      } else changedAt = 0
    } catch (error) { if (!disposed) fail(error) }
    finally { if (!disposed) timer = setTimeout(() => { void poll() }, 1000) }
  }
  void poll()
  return () => { disposed = true; clearTimeout(timer); clear() }
}
