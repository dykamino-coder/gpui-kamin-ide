// React is deliberately separate from the Preact chat tree. Remotes are trusted
// same-context code; this boundary contains rendering failures, not authority.
import { React, ReactDOM, ReactClient, JSXRuntime, JSXDevRuntime, Refresh, reactVersion } from './preview-react'
import { init, registerRemotes, loadRemote } from '@module-federation/enhanced/runtime'
import { captureRuntime, shareRefresh, connectHot, supportsHot, type RemoteRequire } from './preview-hot'
import { prepareEntry } from './preview-entry'

export interface RemoteDescriptor {
  name: string; entry: string; exposedModule: string; type: 'global' | 'module'
  props: Record<string, unknown>; revision: number
}
export type Mounted = { update?(props: Record<string, unknown>): void; unmount(): void; hot?: boolean }
type RemoteModule = {
  default?: React.ComponentType<Record<string, unknown>>
  mount?(element: HTMLElement, props: Record<string, unknown>): Mounted | (() => void) | Promise<Mounted | (() => void)>
}
const version = reactVersion
// MF force registration evicts its container cache, but classic webpack keeps
// replayable chunk factories on window. Retire only chunk globals introduced by
// this remote, so a rebuilt entry cannot replay the previous exposed module.
const chunkGlobals = new Map<string, Map<string, unknown>>()
const isChunkGlobal = (key: string) => /^(webpack|rspack)Chunk/.test(key)
const pending = new Map<string, { before: Set<string>; runtimes: RemoteRequire[]; cleanups: (() => void)[] }>()
const shared = (lib: object) => ({ version, lib: () => lib,
  shareConfig: { singleton: true, requiredVersion: version, strictVersion: true } })
init({ name: 'kamin_bridge_preview_host', remotes: [], plugins: [{ name: 'kamin-preview-refresh',
  afterLoadEntry({ remoteInfo }) {
    const context = pending.get(remoteInfo.name)
    if (!context) return
    const globals = globalThis as unknown as Record<string, unknown>
    for (const key of Object.keys(globals)) {
      if (context.before.has(key) || !isChunkGlobal(key)) continue
      captureRuntime(globals[key], require => {
        if (!context.runtimes.includes(require)) {
          context.runtimes.push(require)
          context.cleanups.push(shareRefresh(require, remoteInfo.name, Refresh))
        }
      })
    }
  },
}], shared: {
  react: shared(React), 'react-dom': shared(ReactDOM),
  'react-dom/client': shared(ReactClient), 'react/jsx-runtime': shared(JSXRuntime),
  'react/jsx-dev-runtime': shared(JSXDevRuntime),
} })

class PreviewBoundary extends React.Component<{ children: React.ReactNode; failed(error: Error): void }, { error?: Error }> {
  state: { error?: Error } = {}
  static getDerivedStateFromError(error: Error) { return { error } }
  componentDidCatch(error: Error) { this.props.failed(error) }
  render() { return this.state.error ? null : this.props.children }
}

export async function renderRemote(element: HTMLElement, descriptor: RemoteDescriptor, build: string,
  failed: (error: Error) => void, applied: () => void, invalidated: () => void): Promise<Mounted> {
  const entry = new URL(descriptor.entry, location.origin)
  if (entry.origin !== location.origin || !entry.pathname.startsWith(`/preview/remotes/${descriptor.name}/`)) {
    throw new Error('Remote entry must use the authenticated workspace proxy')
  }
  entry.searchParams.set('build', build)
  entry.searchParams.set('revision', String(descriptor.revision))
  const globals = globalThis as unknown as Record<string, unknown>
  for (const [key, value] of chunkGlobals.get(descriptor.name) ?? []) {
    if (globals[key] === value) delete globals[key]
  }
  const before = new Set(Object.keys(globals))
  const context = { before, runtimes: [] as RemoteRequire[], cleanups: [] as (() => void)[] }
  const response = await fetch(entry, { credentials: 'same-origin', cache: 'no-store' })
  if (!response.ok) throw new Error('Preview dev server is unavailable')
  context.cleanups.push(prepareEntry(await response.text(), globals, require => {
    if (context.runtimes.includes(require)) return
    context.runtimes.push(require)
    context.cleanups.push(shareRefresh(require, descriptor.name, Refresh))
  }))
  pending.set(descriptor.name, context)
  registerRemotes([{ name: descriptor.name, entry: entry.href, type: descriptor.type }], { force: true })
  let module: RemoteModule | null
  try { module = await loadRemote<RemoteModule>(`${descriptor.name}/${descriptor.exposedModule.slice(2)}`) }
  catch (error) { context.cleanups.forEach(cleanup => cleanup()); throw error }
  finally {
    pending.delete(descriptor.name)
    chunkGlobals.set(descriptor.name, new Map(Object.keys(globals)
      .filter(key => !before.has(key) && isChunkGlobal(key) && Array.isArray(globals[key]))
      .map(key => [key, globals[key]])))
  }
  const cleanup = () => context.cleanups.splice(0).reverse().forEach(dispose => dispose())
  const managed = (mounted: Mounted): Mounted => ({ update: mounted.update?.bind(mounted), hot: context.runtimes.some(supportsHot),
    unmount() { cleanup(); mounted.unmount() } })
  try {
    for (const require of context.runtimes) context.cleanups.push(connectHot(require, descriptor.name, applied, invalidated))
    if (!module) throw new Error('The remote did not expose a module')
    if (typeof module.mount === 'function') {
      const mounted = await module.mount(element, descriptor.props)
      if (typeof mounted === 'function') return managed({ unmount: mounted })
      if (!mounted || typeof mounted.unmount !== 'function') throw new Error('mount() must return cleanup or { unmount(), update?() }')
      return managed(mounted)
    }
    if (!module.default) throw new Error('Expose a default React component or mount(element, props)')
    const root = ReactClient.createRoot(element)
    const render = (props: Record<string, unknown>) => root.render(React.createElement(PreviewBoundary, { failed,
      children: React.createElement(module.default!, props) }))
    render(descriptor.props)
    return managed({ update: render, unmount: () => root.unmount() })
  } catch (error) {
    cleanup()
    throw error
  }
}
