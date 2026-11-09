// React is deliberately separate from the Preact chat tree. Remotes are trusted
// same-context code; this boundary contains rendering failures, not authority.
import * as React from 'react'
import * as ReactDOM from 'react-dom'
import * as ReactClient from 'react-dom/client'
import * as JSXRuntime from 'react/jsx-runtime'
import { init, registerRemotes, loadRemote } from '@module-federation/enhanced/runtime'

export interface RemoteDescriptor {
  name: string; entry: string; exposedModule: string; type: 'global' | 'module'
  props: Record<string, unknown>; revision: number
}
type Mounted = { update?(props: Record<string, unknown>): void; unmount(): void }
type RemoteModule = {
  default?: React.ComponentType<Record<string, unknown>>
  mount?(element: HTMLElement, props: Record<string, unknown>): Mounted | (() => void) | Promise<Mounted | (() => void)>
}
const version = '19.3.1'
// MF force registration evicts its container cache, but classic webpack keeps
// replayable chunk factories on window. Retire only chunk globals introduced by
// this remote, so a rebuilt entry cannot replay the previous exposed module.
const chunkGlobals = new Map<string, Map<string, unknown>>()
const shared = (lib: object) => ({ version, lib: () => lib,
  shareConfig: { singleton: true, requiredVersion: version, strictVersion: true } })
init({ name: 'kamin_bridge_preview_host', remotes: [], shared: {
  react: shared(React), 'react-dom': shared(ReactDOM),
  'react-dom/client': shared(ReactClient), 'react/jsx-runtime': shared(JSXRuntime),
} })

class PreviewBoundary extends React.Component<{ children: React.ReactNode; failed(error: Error): void }, { error?: Error }> {
  state: { error?: Error } = {}
  static getDerivedStateFromError(error: Error) { return { error } }
  componentDidCatch(error: Error) { this.props.failed(error) }
  render() { return this.state.error ? null : this.props.children }
}

export async function renderRemote(element: HTMLElement, descriptor: RemoteDescriptor, build: string,
  failed: (error: Error) => void): Promise<Mounted> {
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
  registerRemotes([{ name: descriptor.name, entry: entry.href, type: descriptor.type }], { force: true })
  let module: RemoteModule | null
  try { module = await loadRemote<RemoteModule>(`${descriptor.name}/${descriptor.exposedModule.slice(2)}`) }
  finally {
    chunkGlobals.set(descriptor.name, new Map(Object.keys(globals)
      .filter(key => !before.has(key) && key.startsWith('webpackChunk') && Array.isArray(globals[key]))
      .map(key => [key, globals[key]])))
  }
  if (!module) throw new Error('The remote did not expose a module')
  if (typeof module.mount === 'function') {
    const mounted = await module.mount(element, descriptor.props)
    if (typeof mounted === 'function') return { unmount: mounted }
    if (!mounted || typeof mounted.unmount !== 'function') throw new Error('mount() must return cleanup or { unmount(), update?() }')
    return mounted
  }
  if (!module.default) throw new Error('Expose a default React component or mount(element, props)')
  const root = ReactClient.createRoot(element)
  const render = (props: Record<string, unknown>) => root.render(React.createElement(PreviewBoundary, { failed,
    children: React.createElement(module.default!, props) }))
  render(descriptor.props)
  return { update: render, unmount: () => root.unmount() }
}
