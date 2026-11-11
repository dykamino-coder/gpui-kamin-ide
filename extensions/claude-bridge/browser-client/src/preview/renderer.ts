// Module Federation remote rendering with host-owned hot updates. React and the
// federation runtime are injected: the embedding page supplies its own React 19
// singleton, so remotes share it, and this package has no runtime dependencies.
// Remotes are trusted same-context code; this boundary contains rendering
// failures, not authority.
import { captureRuntime, shareRefresh, connectHot, supportsHot, type RefreshRuntime, type RemoteRequire } from './hot'
import { prepareEntry } from './entry'
import type { PreviewDescriptor } from '../types'

/* eslint-disable @typescript-eslint/no-explicit-any -- React is injected structurally */
export interface PreviewReact {
  /** The `react` module namespace. */
  React: { createElement(type: any, props?: any, ...children: any[]): any; Component: new (props: any) => any }
  ReactDOM: object
  /** `react-dom/client`. */
  ReactClient: { createRoot(element: HTMLElement): { render(node: unknown): void; unmount(): void } }
  JSXRuntime: object
  JSXDevRuntime: object
  /** `react-refresh/runtime`, injected into the global hook before ReactDOM loads. */
  Refresh: RefreshRuntime
  /** Exact React version every remote must share. */
  version: string
}
/** `init`, `registerRemotes` and `loadRemote` from `@module-federation/enhanced/runtime`. */
export interface FederationRuntime {
  init(options: any): unknown
  registerRemotes(remotes: { name: string; entry: string; type: 'global' | 'module' }[], options: { force: boolean }): void
  loadRemote<T>(id: string): Promise<T | null>
}
/* eslint-enable @typescript-eslint/no-explicit-any */

export interface MountedPreview { update?(props: Record<string, unknown>): void; unmount(): void; hot?: boolean }
export type RenderRemote = (element: HTMLElement, descriptor: PreviewDescriptor, build: string,
  failed: (error: Error) => void, applied: () => void, invalidated: () => void) => Promise<MountedPreview>
type RemoteModule = {
  default?: unknown
  mount?(element: HTMLElement, props: Record<string, unknown>): MountedPreview | (() => void) | Promise<MountedPreview | (() => void)>
}

// MF force registration evicts its container cache, but classic webpack keeps
// replayable chunk factories on window. Retire only chunk globals introduced by
// this remote, so a rebuilt entry cannot replay the previous exposed module.
const isChunkGlobal = (key: string) => /^(webpack|rspack)Chunk/.test(key)

/** Initialise the federation host once per page and return the remote renderer. */
export function createPreviewRenderer(options: { react: PreviewReact; federation: FederationRuntime; hostName?: string }): RenderRemote {
  const { React, ReactDOM, ReactClient, JSXRuntime, JSXDevRuntime, Refresh, version } = options.react
  const { init, registerRemotes, loadRemote } = options.federation
  const chunkGlobals = new Map<string, Map<string, unknown>>()
  const pending = new Map<string, { before: Set<string>; runtimes: RemoteRequire[]; cleanups: (() => void)[] }>()
  const shared = (lib: object) => ({ version, lib: () => lib,
    shareConfig: { singleton: true, requiredVersion: version, strictVersion: true } })
  init({ name: options.hostName ?? 'kamin_bridge_preview_host', remotes: [], plugins: [{ name: 'kamin-preview-refresh',
    afterLoadEntry({ remoteInfo }: { remoteInfo: { name: string } }) {
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

  class PreviewBoundary extends React.Component {
    declare props: { children: unknown; failed(error: Error): void }
    state: { error?: Error } = {}
    static getDerivedStateFromError(error: Error) { return { error } }
    componentDidCatch(error: Error) { this.props.failed(error) }
    render() { return this.state.error ? null : this.props.children }
  }

  return async function renderRemote(element, descriptor, build, failed, applied, invalidated) {
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
    const managed = (mounted: MountedPreview): MountedPreview => ({ update: mounted.update?.bind(mounted), hot: context.runtimes.some(supportsHot),
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
      const component = module.default
      const render = (props: Record<string, unknown>) => root.render(React.createElement(PreviewBoundary, { failed,
        children: React.createElement(component, props) }))
      render(descriptor.props)
      return managed({ update: render, unmount: () => root.unmount() })
    } catch (error) {
      cleanup()
      throw error
    }
  }
}
