// Framework-agnostic client of one workspace host: the same socket and frame
// transport as the bundled browser UI, plus a typed facade over the channels
// an embedding chat UI needs. Raw invoke/send/on remain available for the rest
// (see the channel tables in webview/src/lib/bridge-channels.ts).
import { createBrowserSocket, type BridgeSocket, type BridgeSocketLike, type ConnectionState } from './socket'
import { createBridgeTransport, type BridgeTransport, type Dispose } from './transport'
import type {
  Agent, BrowseProgress, CreatedDirectory, CreatedSkill, CreateTabOptions, ElicitationRequest, HookDraft, HookEntry, HooksByEvent, HookTemplate,
  HookTestResult, InstalledPlugin, JsonlEntry, JsonlStatus, Marketplace, MarketplacePlugin, MarketplaceRefresh, MarketplaceUpdate,
  McpActivity, McpCallResult, McpPromptInfo, McpRegistryResult, McpResourceInfo, McpServer, McpServerInput, McpTestResult,
  MonitorEvent, MonitorInfo, MonitorLine, PendingPluginHookApproval, PermissionDecision, PermissionRequest, PluginContents,
  PluginOptions, PreviewState, RestartResult, SavedSession, SessionActivity, SessionUsage, Skill, StreamingDelta, SubagentEntries,
  Tab, TabId, ToolPermissionMode, UserQuestion,
} from './types'

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
  /** Live appends between whole transcript entries; only while the server streams for this credential. */
  onStreamingDelta(listener: (tab: TabId, delta: StreamingDelta) => void): Dispose
  onActivity(listener: (tab: TabId, activity: SessionActivity) => void): Dispose
  /** Model, effort, context fill and plan limits for a status line. */
  onUsage(listener: (tab: TabId, usage: SessionUsage) => void): Dispose
  /** Conversations of this workspace that resumeSession() can reopen. */
  savedSessions(): Promise<SavedSession[]>

  /** Approval of the workspace tools the agent calls (depends on setToolPermissionMode). */
  onPermissionRequest(listener: (tab: TabId, request: PermissionRequest) => void): Dispose
  respondPermission(requestId: string, decision: PermissionDecision): void
  onQuestion(listener: (tab: TabId, question: UserQuestion) => void): Dispose
  answerQuestion(requestId: string, answer: string): void
  /** MCP elicitations and other structured prompts. */
  onElicitation(listener: (tab: TabId, request: ElicitationRequest) => void): Dispose
  respondElicitation(requestId: string, action: 'accept' | 'deny' | 'dismiss', content?: Record<string, unknown>): void

  /** CLI model alias or id, e.g. 'haiku', 'opus', 'default'. Applied when the session is idle. */
  setModel(tab: TabId, model: string): void
  setEffort(tab: TabId, effort: string): void
  setToolPermissionMode(tab: TabId, mode: ToolPermissionMode): void

  /** Stores bytes in the workspace and returns the path to append to a prompt, one path per line. */
  saveAttachment(base64: string, name: string): Promise<string>
  saveImage(base64: string, mimeType: 'image/png' | 'image/jpeg'): Promise<string>

  /** Files of the workspace container. Relative paths resolve against the workspace root. */
  readonly files: {
    /** Creates one directory (`file-tree:create-dir`). An existing directory resolves with
     * `created: false`; a missing parent or a file at the path rejects (ENOENT, EEXIST). */
    createDirectory(path: string): Promise<CreatedDirectory>
  }

  /** Subagent transcripts are sent on demand: select one, then receive its entries; null stops. */
  selectSubagent(tab: TabId, agentId: string | null): void
  onSubagentEntries(listener: (entries: SubagentEntries) => void): Dispose

  /** Plugin monitors started for open sessions by the host. */
  readonly monitors: {
    list(): Promise<MonitorInfo[]>
    log(id: string): Promise<MonitorLine[]>
    onEvent(listener: (event: MonitorEvent) => void): Dispose
  }

  /** MCP servers the workspace host connects for its sessions. */
  readonly mcp: {
    servers(): Promise<McpServer[]>
    /** False until the host has finished connecting the configured servers. */
    ready(): Promise<boolean>
    addServer(config: McpServerInput): Promise<McpServer | null>
    /** Null when the server is unknown, comes from a plugin or project file, or the new name is taken. */
    updateServer(id: string, patch: Partial<McpServerInput>): Promise<McpServer | null>
    removeServer(id: string): Promise<void>
    setServerEnabled(id: string, enabled: boolean): Promise<void>
    testServer(id: string): Promise<McpTestResult>
    oauthStatus(id: string): Promise<{ connected: boolean }>
    oauthDisconnect(id: string): Promise<{ ok: boolean }>
    resources(id: string): Promise<McpResourceInfo[]>
    readResource(id: string, uri: string): Promise<McpCallResult>
    prompts(id: string): Promise<McpPromptInfo[]>
    getPrompt(id: string, name: string, args?: Record<string, string>): Promise<McpCallResult>
    registry(): Promise<McpRegistryResult>
    onServersChanged(listener: (servers: McpServer[]) => void): Dispose
    /** Tool calls the host executes for a session, as they start and finish. */
    onActivity(listener: (tab: TabId, activity: McpActivity) => void): Dispose
  }

  readonly skills: {
    list(): Promise<Skill[]>
    /** Creates a project skill (user skill when the workspace has no project settings). */
    create(name: string, content: string): Promise<CreatedSkill>
    read(path: string): Promise<string | null>
    /** Project and user skills only. */
    remove(path: string): Promise<void>
  }

  readonly agents: {
    list(): Promise<Agent[]>
  }

  readonly plugins: {
    installed(): Promise<InstalledPlugin[]>
    contents(pluginDir: string): Promise<PluginContents | null>
    install(name: string, marketplace: string, pluginPath: string): Promise<void>
    uninstall(name: string, marketplace: string): Promise<void>
    /** `pluginId` is `<name>@<marketplace>`. */
    setEnabled(pluginId: string, enabled: boolean): Promise<RestartResult>
    options(pluginId: string): Promise<PluginOptions>
    saveOptions(pluginId: string, values: Record<string, unknown>): Promise<RestartResult>
    marketplaces(): Promise<Marketplace[]>
    /** `source` is a git URL, `owner/repo`, or a directory inside the workspace container. */
    addMarketplace(name: string, source: string, auth?: { username?: string; token?: string }): Promise<Marketplace>
    removeMarketplace(name: string): Promise<void>
    updateMarketplace(name: string): Promise<MarketplaceUpdate>
    refreshMarketplaces(): Promise<{ ok: boolean; results: MarketplaceRefresh[] }>
    browse(marketplace: string): Promise<MarketplacePlugin[]>
    onMarketplaceUpdated(listener: (update: MarketplaceRefresh) => void): Dispose
    onBrowseProgress(listener: (progress: BrowseProgress) => void): Dispose
    onHooksAwaitingApproval(listener: (pending: PendingPluginHookApproval) => void): Dispose
  }

  readonly hooks: {
    /** User hooks; positions in the result address save() and remove(). */
    list(): Promise<HooksByEvent>
    /** User, project and plugin hooks with their source. */
    listAll(cwd?: string): Promise<HookEntry[]>
    library(): Promise<HookTemplate[]>
    installTemplate(templateId: string): Promise<RestartResult>
    save(draft: HookDraft): Promise<RestartResult>
    remove(event: string, matcherIdx: number, handlerIdx: number): Promise<RestartResult>
    /** Runs the handler once against `mockPayload` without storing it. */
    test(draft: Omit<HookDraft, 'index'> & { mockPayload?: Record<string, unknown> }): Promise<HookTestResult>
    pendingPluginApprovals(): Promise<PendingPluginHookApproval[]>
    pluginApproval(pluginId: string): Promise<{ approved: boolean; hashes: string[] }>
    setPluginApproval(pluginId: string, hashes: string[]): Promise<RestartResult>
  }

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
    onStreamingDelta: listener => on('streaming-delta', (tab, delta) => listener(tab as TabId, delta as StreamingDelta)),
    onActivity: listener => on('session:activity', (tab, activity) => listener(tab as TabId, activity as SessionActivity)),
    onUsage: listener => on('session:usage', (tab, usage) => listener(tab as TabId, usage as SessionUsage)),
    savedSessions: () => client.invoke<SavedSession[]>('session:get-saved'),

    onPermissionRequest: listener => on('permission-request', (tab, request) => listener(tab as TabId, request as PermissionRequest)),
    respondPermission(requestId, decision) { live().snd('permission-response', requestId, decision) },
    onQuestion: listener => on('ask-user-question', (tab, question) => listener(tab as TabId, question as UserQuestion)),
    answerQuestion(requestId, answer) { live().snd('ask-user-response', requestId, answer) },
    onElicitation: listener => on('elicitation-request', (tab, request) => listener(tab as TabId, request as ElicitationRequest)),
    respondElicitation(requestId, action, content) { live().snd('elicitation-response', requestId, action, ...(content ? [content] : [])) },

    setModel(tab, model) { live().snd('change-model', tab, model) },
    setEffort(tab, effort) { live().snd('change-effort', tab, effort) },
    setToolPermissionMode(tab, mode) { live().snd('change-permission-mode', tab, mode) },

    saveAttachment: (base64, name) => client.invoke<string>('save-dropped-file', base64, name),
    saveImage: (base64, mimeType) => client.invoke<string>('save-clipboard-image', base64, mimeType),

    files: {
      createDirectory: path => client.invoke<CreatedDirectory>('file-tree:create-dir', path),
    },

    selectSubagent(tab, agentId) { live().snd('jsonl:subagent-select', tab, agentId) },
    onSubagentEntries: listener => on('jsonl-subagent-entries', (tab, agentName, entries, agentId) => listener({
      tab: tab as TabId, agentName: String(agentName ?? ''), entries: Array.isArray(entries) ? entries : [],
      ...(typeof agentId === 'string' ? { agentId } : {}),
    })),

    monitors: {
      list: () => client.invoke<MonitorInfo[]>('monitors:list'),
      log: id => client.invoke<MonitorLine[]>('monitors:get-log', id),
      onEvent(listener) {
        const stops = [
          on('monitor:started', monitor => listener({ type: 'started', monitor: monitor as Extract<MonitorEvent, { type: 'started' }>['monitor'] })),
          on('monitor:output', payload => { const { id, entry } = payload as { id: string; entry: MonitorLine }; listener({ type: 'output', id, line: entry }) }),
          on('monitor:status', payload => listener({ type: 'status', ...(payload as { id: string; status: MonitorInfo['status']; exitCode?: number | null }) })),
          on('monitor:stopped', payload => listener({ type: 'stopped', id: (payload as { id: string }).id })),
          on('monitor:error', payload => listener({ type: 'error', ...(payload as { tabId: TabId; pluginId: string; monitorName: string; error: string }) })),
        ]
        return () => { for (const stop of stops) stop() }
      },
    },

    mcp: {
      servers: () => client.invoke<McpServer[]>('mcp:get-servers'),
      ready: () => client.invoke<boolean>('mcp:status'),
      addServer: config => client.invoke<McpServer | null>('mcp:add-server', config),
      updateServer: (id, patch) => client.invoke<McpServer | null>('mcp:update-server', id, patch),
      removeServer: id => client.invoke<void>('mcp:remove-server', id),
      setServerEnabled: (id, enabled) => client.invoke<void>('mcp:toggle-server', id, enabled),
      testServer: id => client.invoke<McpTestResult>('mcp:test-server', id),
      oauthStatus: id => client.invoke<{ connected: boolean }>('mcp:oauth-status', id),
      oauthDisconnect: id => client.invoke<{ ok: boolean }>('mcp:oauth-disconnect', id),
      resources: id => client.invoke<McpResourceInfo[]>('mcp:get-resources', id),
      readResource: (id, uri) => client.invoke<McpCallResult>('mcp:read-resource', id, uri),
      prompts: id => client.invoke<McpPromptInfo[]>('mcp:get-prompts', id),
      getPrompt: (id, name, args) => client.invoke<McpCallResult>('mcp:get-prompt', id, name, ...(args ? [args] : [])),
      registry: () => client.invoke<McpRegistryResult>('mcp:registry-list'),
      onServersChanged: listener => on('mcp-servers-changed', servers => listener(Array.isArray(servers) ? servers as McpServer[] : [])),
      onActivity: listener => on('mcp-activity', (tab, activity) => listener(tab as TabId, activity as McpActivity)),
    },

    skills: {
      list: () => client.invoke<Skill[]>('skills:list'),
      create: (name, content) => client.invoke<CreatedSkill>('skills:create', name, content),
      read: path => client.invoke<string | null>('skills:read', path),
      remove: path => client.invoke<void>('skills:delete', path),
    },

    agents: {
      list: () => client.invoke<Agent[]>('agents:list'),
    },

    plugins: {
      installed: () => client.invoke<InstalledPlugin[]>('plugins:list-installed'),
      contents: pluginDir => client.invoke<PluginContents | null>('plugins:get-contents', pluginDir),
      install: (name, marketplace, pluginPath) => client.invoke<void>('plugins:install', name, marketplace, pluginPath),
      uninstall: (name, marketplace) => client.invoke<void>('plugins:uninstall', name, marketplace),
      setEnabled: (pluginId, enabled) => client.invoke<RestartResult>('plugins:set-enabled', pluginId, enabled),
      options: pluginId => client.invoke<PluginOptions>('plugins:get-options-schema', pluginId),
      saveOptions: (pluginId, values) => client.invoke<RestartResult>('plugins:save-options', pluginId, values),
      marketplaces: () => client.invoke<Marketplace[]>('plugins:list-marketplaces'),
      addMarketplace: (name, source, auth) => client.invoke<Marketplace>('plugins:add-marketplace', name, source, ...(auth ? [auth] : [])),
      removeMarketplace: name => client.invoke<void>('plugins:remove-marketplace', name),
      updateMarketplace: name => client.invoke<MarketplaceUpdate>('plugins:update-marketplace', name),
      refreshMarketplaces: () => client.invoke<{ ok: boolean; results: MarketplaceRefresh[] }>('plugins:refresh-all-marketplaces'),
      browse: marketplace => client.invoke<MarketplacePlugin[]>('plugins:browse-marketplace', marketplace),
      onMarketplaceUpdated: listener => on('marketplaces:updated', update => listener(update as MarketplaceRefresh)),
      onBrowseProgress: listener => on('plugins:browse-progress', progress => listener(progress as BrowseProgress)),
      onHooksAwaitingApproval: listener => on('plugin-hooks:awaiting-approval', pending => listener(pending as PendingPluginHookApproval)),
    },

    hooks: {
      list: async () => (await client.invoke<{ hooks?: HooksByEvent }>('hooks:list'))?.hooks ?? {},
      listAll: async cwd => (await client.invoke<{ hooks?: HookEntry[] }>('hooks:list-all', ...(cwd ? [cwd] : [])))?.hooks ?? [],
      library: async () => (await client.invoke<{ templates?: HookTemplate[] }>('hooks:get-library'))?.templates ?? [],
      installTemplate: templateId => client.invoke<RestartResult>('hooks:install-template', templateId),
      save: draft => client.invoke<RestartResult>('hooks:save', draft),
      remove: (event, matcherIdx, handlerIdx) => client.invoke<RestartResult>('hooks:delete', event, matcherIdx, handlerIdx),
      test: draft => client.invoke<HookTestResult>('hooks:test', draft),
      pendingPluginApprovals: () => client.invoke<PendingPluginHookApproval[]>('hooks:list-pending-plugin-approvals'),
      pluginApproval: pluginId => client.invoke<{ approved: boolean; hashes: string[] }>('hooks:get-plugin-approval', pluginId),
      setPluginApproval: (pluginId, hashes) => client.invoke<RestartResult>('hooks:set-plugin-approval', pluginId, hashes),
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
