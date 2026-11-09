import type * as RefreshAPI from 'react-refresh/runtime'

type RefreshRuntime = typeof RefreshAPI
type Hot = {
  status(): string
  check(apply: boolean): Promise<unknown[] | null>
  addStatusHandler(handler: (status: string) => void): void
  removeStatusHandler(handler: (status: string) => void): void
}
type Module = { exports: unknown; hot?: Hot }
type Factory = (this: unknown, module: Module, ...args: unknown[]) => void
type Interceptor = (options: { id?: string | number; factory: Factory; module: Module }) => void
export type RemoteRequire = {
  i?: Interceptor[]
  c?: Record<string, Module>
  h?: () => string
}

export function supportsHot(require: RemoteRequire): boolean {
  return !!require.h && Object.values(require.c ?? {}).some(module => module.hot)
}
type ChunkQueue = { push(chunk: [string[], object, (require: RemoteRequire) => void]): unknown }

function isRefreshRuntime(value: unknown): value is RefreshRuntime {
  if (!value || typeof value !== 'object') return false
  const api = value as Partial<RefreshRuntime>
  return typeof api.injectIntoGlobalHook === 'function' && typeof api.register === 'function'
    && typeof api.performReactRefresh === 'function' && typeof api.createSignatureFunctionForTransform === 'function'
}

// Use webpack/rspack's module-execution interceptor to recognize the runtime
// API independently of its module ID or plugin-specific loader output. The
// plugin's bundled CJS refresh runtime delegates to the host instance. Keep
// cached exports live too: default dev entries can execute before container.get.
// The remote plugin's boundary/signature logic stays intact.
export function shareRefresh(require: RemoteRequire, name: string, runtime: RefreshRuntime): () => void {
  const shared = { ...runtime,
    register: (type: unknown, id: string) => runtime.register(type, `${name}:${id}`),
    injectIntoGlobalHook: () => {}, // the host injected before loading ReactDOM
  }
  const replace = (module: Module) => {
    if (isRefreshRuntime(module.exports)) {
      if (module.exports === runtime) module.exports = shared
      else Object.assign(module.exports, shared)
    }
  }
  for (const module of Object.values(require.c ?? {})) replace(module)
  const interceptor: Interceptor = options => {
    // The host owns HMR transport and invalidation. Default development module
    // IDs retain the dev-server resource name; skip that startup side effect so
    // it cannot open an unproxied socket or reload the whole workspace page.
    if (typeof options.id === 'string' && /(?:webpack-dev-server|@rspack\/dev-server)[/\\]client[/\\]index\.js(?:\?|$)/.test(options.id)) {
      options.factory = () => {}
      return
    }
    const factory = options.factory
    options.factory = function(module, ...args) {
      factory.call(this, module, ...args)
      replace(module)
    }
  }
  require.i?.push(interceptor)
  return () => {
    const index = require.i?.indexOf(interceptor) ?? -1
    if (index >= 0) require.i!.splice(index, 1)
  }
}

// Container startup differs between federation/Refresh plugins and cannot rely
// on the application's dev-server client being present. Run
// the native HMR check/apply protocol here; the remote plugin alone decides
// whether to refresh a boundary, reset hooks, invalidate, or reject the update.
export function connectHot(require: RemoteRequire, name: string,
  applied: () => void, invalidated: () => void): () => void {
  const hot = Object.values(require.c ?? {}).find(module => module.hot)?.hot
  if (!hot || !require.h) return () => {}
  const url = new URL(`/preview/remotes/${name}/ws`, location.href)
  url.protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  let socket: WebSocket | undefined, reconnect: ReturnType<typeof setTimeout> | undefined
  let disposed = false, checking = false, hash = '', ready = false
  async function update() {
    if (disposed || checking || !ready || !hash || hot!.status() !== 'idle' || require.h!() === hash) return
    checking = true
    const target = hash
    try {
      const modules = await hot!.check(true)
      if (disposed) return
      if (!modules) { invalidated(); return }
      applied()
    } catch { if (!disposed) invalidated() }
    finally {
      checking = false
      // A newer build may have arrived while the previous one was applying.
      if (!disposed && hash !== target) void update()
    }
  }
  const status = (value: string) => { if (value === 'idle') queueMicrotask(() => { void update() }) }
  hot.addStatusHandler(status)
  function connect() {
    if (disposed) return
    socket = new WebSocket(url)
    socket.onmessage = event => {
      let message: { type: string; data?: string }
      try { message = JSON.parse(String(event.data)) } catch { return }
      if (message.type === 'invalid') ready = false
      if (message.type === 'hash') hash = message.data ?? ''
      if (message.type === 'ok' || message.type === 'still-ok') { ready = true; void update() }
    }
    socket.onclose = () => { if (!disposed) reconnect = setTimeout(connect, 1000) }
    socket.onerror = () => socket?.close()
  }
  connect()
  return () => {
    disposed = true
    clearTimeout(reconnect)
    hot.removeStatusHandler(status)
    socket?.close()
  }
}

let sequence = 0
export function captureRuntime(queue: unknown, receive: (require: RemoteRequire) => void): void {
  if (!Array.isArray(queue)) return
  // A runtime chunk callback is part of both webpack and rspack's web chunk
  // loading protocol. It runs before the federation factory is requested.
  ;(queue as unknown as ChunkQueue).push([[`kamin-preview-${++sequence}`], {}, receive])
}
