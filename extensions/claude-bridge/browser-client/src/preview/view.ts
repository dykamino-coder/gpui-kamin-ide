// Framework-agnostic preview element that follows the host's preview state:
// mounts the remote, applies props updates, remounts on a new revision and,
// for dev servers without HMR, after a stable entry rebuild. The host owns
// mount/props and dev-server availability; this only projects them.
import type { PreviewDescriptor, PreviewState } from '../types'
import type { MountedPreview, RenderRemote } from './renderer'

export type PreviewStatus = 'empty' | 'unavailable' | 'mounting' | 'ready' | 'error'
export interface PreviewView {
  /** Container the remote renders into. Place it anywhere; the view owns its children. */
  readonly element: HTMLElement
  readonly descriptor: PreviewDescriptor | null
  readonly status: PreviewStatus
  readonly error: Error | undefined
  /** Fires after every descriptor, status or error change. */
  onChange(listener: () => void): () => void
  /** Remount the current remote. */
  reload(): void
  dispose(): void
}
export interface PreviewViewOptions {
  /** Subscribe to host preview states, e.g. BridgeClient.preview.onState. */
  states(listener: (state: PreviewState) => void): () => void
  render: RenderRemote
  /** Existing container; created otherwise. */
  element?: HTMLElement
  /** Rebuilt entries of non-HMR servers remount after this quiet period. */
  rebuildDelayMs?: number
}

export function createPreviewView(options: PreviewViewOptions): PreviewView {
  const element = options.element ?? document.createElement('div')
  const rebuildDelay = options.rebuildDelayMs ?? 5000
  const listeners = new Set<() => void>()
  let disposed = false, epoch = 0, key = '', props = '', fingerprint = '', latestBuild = ''
  let mounted: MountedPreview | undefined
  let descriptor: PreviewDescriptor | null = null
  let status: PreviewStatus = 'empty'
  let error: Error | undefined
  let rebuild: ReturnType<typeof setTimeout> | undefined

  function changed(): void { for (const listener of [...listeners]) { try { listener() } catch { /* observers cannot stop preview */ } } }
  function set(next: PreviewStatus, failure?: Error): void { status = next; error = failure; changed() }
  function fail(failure: unknown): void { set('error', failure instanceof Error ? failure : new Error('Remote failed')) }
  function clear(): void {
    ++epoch
    try { mounted?.unmount() } catch { /* a remote cleanup cannot stop the page */ }
    mounted = undefined
    element.replaceChildren()
  }
  async function mount(value: PreviewDescriptor, build: string): Promise<void> {
    clear()
    const generation = epoch
    set('mounting')
    const target = document.createElement('div')
    element.append(target)
    try {
      const result = await options.render(target, value, build, fail, () => {},
        () => { if (generation === epoch && descriptor) void mount(descriptor, fingerprint) })
      if (disposed || generation !== epoch) { result.unmount(); return }
      mounted = result
      set('ready')
    } catch (failure) { if (generation === epoch) { target.replaceChildren(); fail(failure) } }
  }
  function cancelRebuild(): void { clearTimeout(rebuild); rebuild = undefined }

  function apply(state: PreviewState): void {
    if (disposed) return
    descriptor = state.descriptor
    if (!descriptor) {
      cancelRebuild()
      if (key) clear()
      key = props = fingerprint = latestBuild = ''
      set('empty')
      return
    }
    if (!state.available || !state.build) { set('unavailable'); return }
    latestBuild = state.build
    const nextKey = `${descriptor.name}:${descriptor.revision}`
    const nextProps = JSON.stringify(descriptor.props)
    if (nextKey !== key) {
      key = nextKey; props = nextProps; fingerprint = state.build
      cancelRebuild()
      void mount(descriptor, state.build)
      return
    }
    if (props !== nextProps) {
      props = nextProps
      if (mounted?.update) { try { mounted.update(descriptor.props) } catch (failure) { fail(failure) } }
      else void mount(descriptor, state.build)
    }
    if (status === 'unavailable' && mounted) set('ready')
    // A native HMR runtime owns its updates and reports invalidation directly.
    // Only servers without HMR use the stable-entry rebuild fallback.
    if (fingerprint === state.build) { cancelRebuild(); return }
    if (mounted?.hot) { fingerprint = state.build; cancelRebuild(); return }
    rebuild ??= setTimeout(() => {
      rebuild = undefined
      if (disposed || !descriptor || fingerprint === latestBuild) return
      fingerprint = latestBuild
      void mount(descriptor, latestBuild)
    }, rebuildDelay)
  }

  const unsubscribe = options.states(apply)
  return {
    element,
    get descriptor() { return descriptor },
    get status() { return status },
    get error() { return error },
    onChange(listener) { listeners.add(listener); return () => { listeners.delete(listener) } },
    reload() { if (descriptor && !disposed) void mount(descriptor, fingerprint) },
    dispose() {
      if (disposed) return
      disposed = true
      unsubscribe()
      cancelRebuild()
      clear()
      listeners.clear()
    },
  }
}
