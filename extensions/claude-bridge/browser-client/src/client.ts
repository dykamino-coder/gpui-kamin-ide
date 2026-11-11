// Framework-agnostic client of one workspace host: the same socket and frame
// transport as the bundled browser UI, plus a typed facade over the channels
// an embedding chat UI needs. Raw invoke/send/on remain available for the rest
// (see the channel tables in webview/src/lib/bridge-channels.ts).
import { createBrowserSocket, type BridgeSocket, type BridgeSocketLike, type ConnectionState } from './socket'
import { createBridgeTransport, type BridgeTransport, type Dispose } from './transport'
import type { CreateTabOptions, JsonlEntry, JsonlStatus, PreviewState, Tab, TabId } from './types'

export interface BridgeClientOptions {
  /** Workspace origin, e.g. https://studio.example/workspace-proxy origin. Defaults to location.origin. */
  origin?: string
  createSocket?: (url: string) => BridgeSocketLike
  /** Keeps the host's logical file view across reloads; sessionStorage by default. */
  fileViewIdentity?: { get(): string | undefined; set(id: string): void }
  /** Re-send `workspace:sync` after a reconnect so subscriptions receive a fresh snapshot. Default true. */
  syncOnReconnect?: boolean
  fetch?: typeof fetch
  /** Rejects sync() when the host sends no snapshot in time. Default 15 s. */
  syncTimeoutMs?: number
}

export interface WorkspaceSnapshot { tabs: Tab[]; active: TabId | null }

export interface BridgeClient {
  readonly connectionState: ConnectionState
  onConnectionState(listener: (state: ConnectionState) => void): Dispose
  /** Opens /bridge with the existing cookie session. Resolves when connected;
   * rejects when the host requires sign-in or closes the connection. */
  connect(): Promise<void>
  disconnect(): void

  /** Raw channels. Invokes are never retried; a disconnect rejects them with an unknown outcome. */
  invoke<T = unknown>(channel: string, ...args: unknown[]): Promise<T>
  send(channel: string, ...args: unknown[]): void
  on(channel: string, listener: (...args: unknown[]) => void): Dispose

  /** Full snapshot: tabs, active tab, then its transcript through the event subscriptions. */
  sync(): Promise<WorkspaceSnapshot>
  tabs(): Tab[]
  activeTabId(): TabId | null
  createTab(options?: CreateTabOptions): Promise<TabId>
  resumeSession(conversationId: string, options?: CreateTabOptions & { label?: string }): Promise<TabId>
  switchTab(id: TabId): void
  closeTab(id: TabId): void
  submitPrompt(tab: TabId, text: string, submissionId?: string): void
  interrupt(tab: TabId): void
  onJsonlEntries(listener: (tab: TabId, entries: JsonlEntry[]) => void): Dispose
  onStreaming(listener: (tab: TabId, entry: JsonlEntry) => void): Dispose
  onStatus(listener: (tab: TabId, status: JsonlStatus) => void): Dispose
  onTabsChanged(listener: (tabs: Tab[], active: TabId | null) => void): Dispose

  readonly preview: {
    /** Last state pushed by the host, or null before the first event. */
    state(): PreviewState | null
    /** Pushed on connect and on every change (`workspace:preview-state`). */
    onState(listener: (state: PreviewState) => void): Dispose
    /** One-off read through GET /preview/state, for hosts older than the event. */
    fetchState(): Promise<PreviewState>
    unmount(): Promise<void>
  }
}

function sessionStorageIdentity(): BridgeClientOptions['fileViewIdentity'] {
  return {
    get() { try { return sessionStorage.getItem('workspace-file-view') ?? undefined } catch { return undefined } },
    set(id) { try { sessionStorage.setItem('workspace-file-view', id) } catch { /* host adopts detached views */ } },
  }
}

export function createBridgeClient(options: BridgeClientOptions = {}): BridgeClient {
  const origin = new URL(options.origin ?? location.origin).origin
  const socketUrl = new URL('/bridge', origin)
  socketUrl.protocol = socketUrl.protocol === 'https:' ? 'wss:' : 'ws:'
  const request = options.fetch ?? ((input: RequestInfo | URL, init?: RequestInit) => fetch(input, init))
  const stateListeners = new Set<(state: ConnectionState) => void>()
  let connectionState: ConnectionState = 'closed'
  let socket: BridgeSocket | undefined
  let transport: BridgeTransport | undefined
  let connected = false
  let waiters: { resolve(): void; reject(error: Error): void }[] = []
  let tabs: Tab[] = []
  let active: TabId | null = null
  let preview: PreviewState | null = null
  // Subscriptions outlive sockets: they are re-attached to each new transport.
  const subscriptions = new Map<number, { channel: string; listener: (...args: unknown[]) => void; dispose?: Dispose }>()
  let nextSubscription = 0

  function on(channel: string, listener: (...args: unknown[]) => void): Dispose {
    const id = ++nextSubscription
    const entry: { channel: string; listener: (...args: unknown[]) => void; dispose?: Dispose } = { channel, listener }
    subscriptions.set(id, entry)
    entry.dispose = transport?.sub(channel, listener)
    return () => { entry.dispose?.(); subscriptions.delete(id) }
  }
  function live(): BridgeTransport {
    if (!transport || !connected) throw new Error('Workspace disconnected; command was not sent')
    return transport
  }
  function detach(): void {
    const current = socket
    socket = undefined
    transport = undefined
    for (const entry of subscriptions.values()) { entry.dispose?.(); entry.dispose = undefined }
    current?.dispose()
  }
  function setState(state: ConnectionState): void {
    connectionState = state
    connected = state === 'connected'
    // Terminal states: the next connect() opens a new socket (after sign-in).
    if (state === 'authentication-required' || (state === 'closed' && socket)) detach()
    if (state === 'connected') { for (const waiter of waiters.splice(0)) waiter.resolve() }
    if (state === 'authentication-required' || state === 'closed') {
      const error = new Error(state === 'closed' ? 'Workspace connection closed' : 'Workspace sign-in required')
      for (const waiter of waiters.splice(0)) waiter.reject(error)
    }
    for (const listener of [...stateListeners]) { try { listener(state) } catch { /* observers cannot stop delivery */ } }
  }

  // Internal state followers; registered once, re-attached with every transport.
  on('workspace:snapshot-start', (list, current) => { if (Array.isArray(list)) tabs = list as Tab[]; active = typeof current === 'string' ? current : null })
  on('tab:list-changed', list => { if (Array.isArray(list)) tabs = list as Tab[] })
  on('tab:switched', id => { if (typeof id === 'string') active = id })
  on('workspace:preview-state', state => { if (state && typeof state === 'object') preview = state as PreviewState })
  on('bridge:reconnected', () => { if (options.syncOnReconnect !== false) transport?.snd('workspace:sync') })

  const client: BridgeClient = {
    get connectionState() { return connectionState },
    onConnectionState(listener) { stateListeners.add(listener); return () => { stateListeners.delete(listener) } },
    connect() {
      if (connected) return Promise.resolve()
      const ready = new Promise<void>((resolve, reject) => { waiters.push({ resolve, reject }) })
      if (!socket) {
        socket = createBrowserSocket({ url: socketUrl.href, createSocket: options.createSocket,
          fileViewIdentity: options.fileViewIdentity ?? sessionStorageIdentity(), onState: setState })
        transport = createBridgeTransport(socket)
        for (const entry of subscriptions.values()) entry.dispose = transport.sub(entry.channel, entry.listener)
        socket.connect()
      }
      return ready
    },
    disconnect() {
      detach()
      setState('closed')
    },
    invoke<T>(channel: string, ...args: unknown[]) {
      try { return live().inv(channel, ...args) as Promise<T> } catch (error) { return Promise.reject(error) }
    },
    send(channel, ...args) { live().snd(channel, ...args) },
    on,

    sync() {
      return new Promise<WorkspaceSnapshot>((resolve, reject) => {
        let snapshot: WorkspaceSnapshot | undefined
        const stop = () => { clearTimeout(timer); start(); end() }
        const timer = setTimeout(() => { stop(); reject(new Error('Workspace snapshot timed out')) }, options.syncTimeoutMs ?? 15_000)
        const start = on('workspace:snapshot-start', (list, current) => {
          snapshot = { tabs: Array.isArray(list) ? list as Tab[] : [], active: typeof current === 'string' ? current : null }
        })
        const end = on('workspace:snapshot-end', () => { if (snapshot) { stop(); resolve(snapshot) } })
        try { live().snd('workspace:sync') } catch (error) { stop(); reject(error) }
      })
    },
    tabs: () => structuredClone(tabs),
    activeTabId: () => active,
    async createTab(tabOptions = {}) {
      const id = await client.invoke<TabId | null>('tab:create', { ...tabOptions })
      if (typeof id !== 'string') throw new Error('The workspace did not create a session')
      return id
    },
    async resumeSession(conversationId, resumeOptions = {}) {
      const { label, ...rest } = resumeOptions
      const id = await client.invoke<TabId | null>('session:resume', { ...rest, conversationId, ...(label ? { sessionLabel: label } : {}) })
      if (typeof id !== 'string') throw new Error('The workspace did not resume the session')
      return id
    },
    switchTab(id) { live().snd('tab:switch', id); active = id },
    closeTab(id) { live().snd('tab:close', id) },
    submitPrompt(tab, text, submissionId) { live().snd('pty-submit-text', tab, text, ...(submissionId ? [submissionId] : [])) },
    interrupt(tab) { live().snd('session:interrupt', tab) },
    onJsonlEntries: listener => on('jsonl-entries', (tab, entries) => listener(tab as TabId, Array.isArray(entries) ? entries as JsonlEntry[] : [])),
    onStreaming: listener => on('streaming-entry', (tab, entry) => listener(tab as TabId, entry as JsonlEntry)),
    onStatus: listener => on('jsonl-status', (tab, status) => listener(tab as TabId, status as JsonlStatus)),
    onTabsChanged(listener) {
      const notify = () => listener(structuredClone(tabs), active)
      const stops = [on('tab:list-changed', notify), on('tab:switched', notify), on('workspace:snapshot-start', notify)]
      return () => { for (const stop of stops) stop() }
    },

    preview: {
      state: () => preview && structuredClone(preview),
      onState(listener) {
        if (preview) listener(structuredClone(preview))
        return on('workspace:preview-state', state => listener(state as PreviewState))
      },
      async fetchState() {
        const response = await request(new URL('/preview/state', origin), { credentials: 'same-origin', cache: 'no-store' })
        if (!response.ok) throw new Error(response.status === 404 ? 'Preview is not configured' : 'Workspace session unavailable')
        return await response.json() as PreviewState
      },
      async unmount() {
        const response = await request(new URL('/preview/unmount', origin), { method: 'POST', credentials: 'same-origin' })
        if (!response.ok) throw new Error('Preview unmount failed')
      },
    },
  }
  return client
}
