// Cookie-authenticated /bridge WebSocket of a workspace host. Shared by the
// bundled browser UI and @kaminide/bridge-browser-client.

/** `authentication-required` (close 4001) and `closed` (4002/4008) are terminal; the rest retry with backoff. */
export type ConnectionState = 'connecting' | 'connected' | 'offline' | 'authentication-required' | 'closed'
/** The subset of the DOM WebSocket the transport uses; tests and other runtimes may supply their own. */
export type BridgeSocketLike = Pick<WebSocket, 'readyState' | 'bufferedAmount' | 'send' | 'close' | 'onopen' | 'onclose' | 'onmessage' | 'onerror'>
/** Frame port between a transport and its host (VS Code webview API shape). */
export interface HostPort {
  postMessage: (msg: unknown) => void
  getState: () => unknown
  setState: (state: unknown) => void
  /** Sockets deliver through a private subscription, never window messages. */
  onHostMessage?: (receive: (frame: unknown) => void) => () => void
  onDisconnect?: (receive: () => void) => () => void
}
export interface BridgeSocketOptions {
  /** Absolute ws:/wss: URL whose path is exactly /bridge. */
  url: string
  createSocket?: (url: string) => BridgeSocketLike
  onState?: (state: ConnectionState) => void
  /** Keeps the host's logical file view across reloads of this document. */
  fileViewIdentity?: { get(): string | undefined; set(id: string): void }
}
const MAX_FRAME = 16 * 1024 * 1024
const MAX_BUFFER = 32 * 1024 * 1024

function hostFrame(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false
  const f = value as Record<string, unknown>
  if (f.kind === 'invoke-reply') return Number.isSafeInteger(f.id) && (f.id as number) > 0
    && typeof f.ok === 'boolean' && (f.generation === undefined || typeof f.generation === 'string')
  return f.kind === 'event' && typeof f.channel === 'string' && f.channel.length <= 256
    && (f.args === undefined || Array.isArray(f.args))
}

export type BridgeSocket = HostPort & Required<Pick<HostPort, 'onHostMessage' | 'onDisconnect'>> & { connect(): void; dispose(): void }

/** Cookie-authenticated same-origin socket. Never buffers or replays commands. */
export function createBrowserSocket(options: BridgeSocketOptions): BridgeSocket {
  const url = new URL(options.url)
  if (!['ws:', 'wss:'].includes(url.protocol) || url.username || url.password || url.search || url.hash
    || url.pathname !== '/bridge') throw new Error('Invalid workspace socket URL')
  const messages = new Set<(frame: unknown) => void>()
  const disconnects = new Set<() => void>()
  const factory = options.createSocket ?? (address => new WebSocket(address))
  let socket: BridgeSocketLike | undefined
  let retry: ReturnType<typeof setTimeout> | undefined
  let deadline: ReturnType<typeof setTimeout> | undefined
  let stopped = false
  let connectedBefore = false
  let delay = 250
  let state: unknown

  function clearDeadline(): void { clearTimeout(deadline); deadline = undefined }
  function lost(): void { for (const receive of disconnects) receive() }
  function schedule(): void {
    options.onState?.('offline')
    retry = setTimeout(connect, delay)
    delay = Math.min(delay * 2, 10_000)
  }
  function connect(): void {
    if (stopped || socket) return
    clearTimeout(retry)
    retry = undefined
    options.onState?.('connecting')
    let current: BridgeSocketLike
    try { current = factory(url.href) } catch { schedule(); return }
    socket = current
    const active = () => !stopped && socket === current
    deadline = setTimeout(() => {
      if (!active() || current.readyState === 1) return
      // Own the transition even when a stalled socket never emits close.
      socket = undefined
      current.close()
      lost()
      schedule()
    }, 10_000)
    current.onopen = () => {
      if (!active()) return
      clearDeadline()
      delay = 250
      options.onState?.('connected')
      if (connectedBefore) for (const receive of messages) receive({ kind: 'event', channel: 'bridge:reconnected', args: [] })
      connectedBefore = true
    }
    current.onmessage = event => {
      if (!active()) return
      try {
        if (typeof event.data !== 'string' || new TextEncoder().encode(event.data).length > MAX_FRAME) throw new Error('Frame size')
        const frame: unknown = JSON.parse(event.data)
        if (!hostFrame(frame)) throw new Error('Frame shape')
        const f = frame as { kind: string; channel?: string; args?: { id?: unknown }[] }
        if (f.kind === 'event' && f.channel === 'workspace:files-state' && typeof f.args?.[0]?.id === 'string') {
          options.fileViewIdentity?.set(f.args[0].id)
        }
        for (const receive of messages) receive(frame)
      } catch { current.close(4002, 'Invalid host frame') }
    }
    current.onerror = () => { /* close or connection deadline owns retry */ }
    current.onclose = event => {
      if (!active()) return
      socket = undefined
      clearDeadline()
      lost()
      if ([4001, 4002, 4008].includes(event.code)) {
        stopped = true
        options.onState?.(event.code === 4001 ? 'authentication-required' : 'closed')
      } else schedule()
    }
  }
  return {
    connect,
    postMessage(frame) {
      if (!socket || socket.readyState !== 1 || stopped) {
        const diagnostic = frame as { kind?: unknown; channel?: unknown } | null
        if (diagnostic?.kind === 'invoke-diagnostic' || (diagnostic?.kind === 'send'
          && typeof diagnostic.channel === 'string' && diagnostic.channel.startsWith('diag:'))) return
        throw new Error('Workspace disconnected; command was not sent')
      }
      const f = frame as { kind?: string; channel?: string } | null
      const identity = options.fileViewIdentity?.get()
      const text = JSON.stringify(f?.kind === 'send' && identity
        ? f.channel === 'workspace:sync' ? { ...f, args: [{ fileViewId: identity }] }
          : f.channel === 'workspace:file-panel' || /^(file-viewer|file-tree):(watch|unwatch)$/.test(f.channel ?? '')
            ? { ...f, fileViewId: identity } : frame
        : frame)
      if (typeof text !== 'string') throw new Error('Invalid workspace frame')
      const size = new TextEncoder().encode(text).length
      if (size > MAX_FRAME || socket.bufferedAmount + size > MAX_BUFFER) throw new Error('Workspace output limit')
      socket.send(text)
    },
    // UI state lives only in this document; credentials never enter this API.
    getState: () => state,
    setState: value => { state = value },
    onHostMessage(receive) { messages.add(receive); return () => { messages.delete(receive) } },
    onDisconnect(receive) { disconnects.add(receive); return () => { disconnects.delete(receive) } },
    dispose() {
      if (stopped && !socket && !retry) return
      stopped = true
      clearTimeout(retry)
      retry = undefined
      clearDeadline()
      const current = socket
      socket = undefined
      current?.close(1000, 'Page ended')
      lost()
      messages.clear()
      disconnects.clear()
    },
  }
}
