// Server URL and connection state
export interface ConnectionConfig {
  serverUrl: string
  token: string
  cwd?: string
  whisperUrl?: string
  whisperToken?: string
  whisperModel?: string
}

export interface ConnectionState {
  status: 'disconnected' | 'connecting' | 'connected' | 'error'
  sessionId?: string
  error?: string
  /** Epoch ms of the scheduled reconnect. The UI counts down to it, which a
   *  message baked at schedule time ("in 8s") cannot do — that number is stale
   *  the moment it is written, so a user watching a long backoff could not tell
   *  a pending retry from a stalled one. */
  nextRetryAt?: number
  /** Which attempt the pending retry will be. Retries never stop, so this is
   *  what shows the loop is alive. */
  retryAttempt?: number
}

// Tab info for sidebar rendering
export interface TabInfo {
  id: string
  cwd: string
  label: string           // "#1", "#2", etc.
  folderName: string      // basename(cwd) for grouping
  createdAt: string       // ISO timestamp
  status: ConnectionState['status']
  /** Mirrored from the tab's ConnectionState so the header can count down to a
   *  pending reconnect (see ConnectionStatusBadge). */
  nextRetryAt?: number
  retryAttempt?: number
  effort?: string         // current effort level (high, low, medium, default)
  model?: string          // current model (opus, sonnet, haiku)
  sessionTitle?: string   // auto-generated topic ("Fix auth bug")
  conversationId?: string // CLI conversation UUID (for resume dedup)
  sessionId?: string      // server-side PTY session ID
  settingsDir?: string    // server-side settings directory path
  pinned?: boolean        // user-pinned tab (persisted per-token)
}

export interface PersistedTabState {
  id: string
  title: string
  cwd: string
  conversationId?: string
  pinned: boolean
  order: number
  /** Position in the top TabsBar strip (linear, ignores folder grouping).
   *  -1 = not placed in strip order yet (fresh tab, falls to the end). */
  topOrder?: number
}

// MCP tool call from server
export interface McpToolCall {
  requestId: string
  toolName: string
  input: Record<string, unknown>
  timestamp: number
}

// MCP tool result to send back
export interface McpToolResult {
  requestId: string
  result: unknown
  durationMs: number
}

// Permission decision
export type PermissionDecision = 'allow_once' | 'allow_session' | 'always' | 'deny'

export interface PermissionRequest {
  requestId: string
  toolName: string
  input: Record<string, unknown>
  category: 'read-only' | 'write' | 'shell' | 'git' | 'network' | 'agent' | 'task' | 'team' | 'ui'
}

// Elicitation (interactive prompt from Claude)
export interface ElicitationRequest {
  requestId: string
  toolName: string
  message: string
  requestedSchema?: Record<string, unknown>
}

export interface ElicitationResponse {
  requestId: string
  action: 'accept' | 'deny' | 'dismiss'
  content?: Record<string, unknown>
}

// Session tree node
export interface TreeNode {
  id: string
  type: 'session' | 'agent' | 'team' | 'teammate'
  label: string
  status: 'active' | 'idle' | 'busy' | 'done' | 'error' | 'exited'
  model?: string
  children: TreeNode[]
  parentId?: string
  folderName?: string
  cwd?: string
  /** Auto-generated session topic from CLI (e.g. "Fix auth bug") */
  sessionTitle?: string
}

// Saved session for resume
export interface SavedSession {
  conversationId: string
  cwd: string
  label: string
  folderName: string
  model: string
  lastActivity: string
  messageCount: number
}

// External MCP server configuration
export interface ExternalMcpServerConfig {
  id: string
  name: string
  type: 'http' | 'stdio' | 'sse' | 'ws'
  // HTTP/SSE/WS: URL endpoint (e.g. http://localhost:8080/mcp or ws://localhost:8080/mcp)
  url?: string
  headers?: Record<string, string>
  // stdio: command + args (e.g. "npx -y @modelcontextprotocol/server-github")
  command?: string
  args?: string[]
  env?: Record<string, string>
  /** Working directory for stdio spawn (mirrors CLI's `workingDirectory`). */
  workingDirectory?: string
  enabled: boolean
  /** True if this server was auto-discovered from local Claude settings */
  autoDiscovered?: boolean
  /** Source path of the .mcp.json file (for auto-discovered servers) */
  sourcePath?: string
  /** Where this server config came from */
  sourceType?: 'plugin' | 'claude-config' | 'project'
  /** Plugin name (for plugin-sourced servers) */
  pluginName?: string
  /** Original server key inside the plugin before bridge-internal scoping. */
  pluginServerName?: string
  /** Marketplace name (for plugin-sourced servers) */
  marketplace?: string
  /** Human-readable description */
  description?: string
  /** OAuth 2.0 configuration for HTTP/SSE servers. If present, the Authorization
   *  header is populated with a Bearer token obtained via the authorization-code
   *  flow (PKCE). Tokens are stored encrypted per server id. */
  oauth?: {
    /** Either the issuer URL or a full `.well-known/oauth-authorization-server` URL. */
    authServerUrl: string
    clientId: string
    /** Optional — confidential clients only. Omit for public (PKCE-only) clients. */
    clientSecret?: string
    scope?: string
    /** Fixed callback port. 0 or omitted = pick any free port. */
    callbackPort?: number
  }
}

export interface McpToolInfo {
  name: string
  description?: string
}

export interface ExternalMcpServerInfo extends ExternalMcpServerConfig {
  status: 'disconnected' | 'connecting' | 'connected' | 'error'
  tools: string[]
  toolInfos?: McpToolInfo[]
  resources?: McpResourceInfo[]
  resourceTemplates?: McpResourceTemplateInfo[]
  prompts?: McpPromptInfo[]
  error?: string
}

export interface McpResourceInfo {
  uri: string
  name: string
  description?: string
  mimeType?: string
}

export interface McpResourceTemplateInfo {
  uriTemplate: string
  name: string
  description?: string
  mimeType?: string
}

export interface McpPromptInfo {
  name: string
  description?: string
  arguments?: { name: string; description?: string; required?: boolean }[]
}

/** Item returned by the official Anthropic MCP registry. */
export interface McpRegistryEntry {
  name: string
  description?: string
  repository?: string
  homepage?: string
  /** Installation hint: stdio command, http/sse URL, etc. */
  install?: {
    type: 'stdio' | 'http' | 'sse' | 'ws'
    command?: string
    args?: string[]
    url?: string
  }
}

// Marketplace info
export interface MarketplaceInfo {
  name: string
  source: { source: string; url?: string; repo?: string; path?: string }
  installLocation: string
  lastUpdated: string
  /** If false, skipped by background auto-refresh on app start. Missing ⇒ true. */
  autoUpdate?: boolean
  isAnthropicOfficial: boolean
}

// Plugin from a marketplace
export interface MarketplacePlugin {
  name: string
  version: string
  description: string
  author: string
  marketplace: string
  keywords: string[]
  pluginPath: string
  isInstalled: boolean
  hasCommands: boolean
}

// Kamin webview bridge API. The historical Electron name remains below as a
// compatibility type only.
export interface KaminBridgeApi {
  // ─── Tabs ──────────────────────────────────────────────
  createTab(config: ConnectionConfig): Promise<string>
  closeTab(tabId: string): void
  disconnectTab(tabId: string): void
  reconnectTab(tabId: string): void
  /** VSIX-only: ask the CLI to regenerate this session's AI title from chat. */
  regenerateTitle(tabId: string): void
  switchTab(tabId: string): void
  listTabs(): Promise<TabInfo[]>
  getActiveTab(): Promise<string | null>
  onTabCreated(callback: (tab: TabInfo) => void): () => void
  onTabClosed(callback: (tabId: string) => void): () => void
  onTabSwitched(callback: (tabId: string) => void): () => void
  onTabListChanged(callback: (tabs: TabInfo[]) => void): () => void
  saveTabsState(tabs: PersistedTabState[]): Promise<{ ok: boolean }>
  restoreTabsState(): Promise<PersistedTabState[]>

  // ─── Connection (legacy, for settings panel) ──────────
  connect(config: ConnectionConfig): void
  disconnect(): void
  getConnectionState(): Promise<ConnectionState>
  onConnectionStateChange(callback: (tabId: string, state: ConnectionState) => void): () => void
  getLayout(): Promise<{
    sidebarWidth?: number
    sidebarWidthRatio?: number
    filePanelVisible?: boolean
    filePanelWidth?: number
    filePanelWidthRatio?: number
    filePanelBottomVisible?: boolean
    filePanelSplit?: number
  }>
  setLayout(patch: {
    sidebarWidth?: number
    sidebarWidthRatio?: number
    filePanelVisible?: boolean
    filePanelWidth?: number
    filePanelWidthRatio?: number
    filePanelBottomVisible?: boolean
    filePanelSplit?: number
  }): void

  getNotifications(): Promise<{ backgroundToasts: boolean }>
  setNotifications(patch: { backgroundToasts?: boolean }): void

  getTerminalPrefs(): Promise<{ useConptyDll: boolean }>
  setTerminalPrefs(patch: { useConptyDll?: boolean }): Promise<void>
  setTheme(theme: 'dark' | 'light'): void
  testNotification(variant?: 'single' | 'burst' | 'sticky'): Promise<{ ok: boolean }>
  onToastRoute(callback: (payload: { tabId: string; requestId?: string }) => void): () => void
  /** Панель Tools попросила раскрыть применения тула в чате. */
  onToolUsageOpen(callback: (toolName: string) => void): () => void
  /** Применения тула из полного кэша хоста (панель Tools, inline-просмотр). */
  getToolUsageEntries(tabId: string, toolName: string): Promise<unknown[]>
  /** Из панели Tools: открыть fullscreen применений тула (relay через extension). */
  openToolUsage(toolName: string): void

  pathExists(p: string): Promise<boolean>

  /** OS clipboard write via the host (works from sandboxed iframes where
   *  navigator.clipboard requires document focus). Channel: write-clipboard. */
  writeClipboard(text: string): Promise<void>

  // ─── App log collector (Customize → Logs) ─────────────
  listLogs(): Promise<Array<{ ts: number; level: 'error' | 'warn' | 'info'; source: string; message: string; stack?: string }>>
  clearLogs(): Promise<void>
  onLogEntry(callback: (entry: { ts: number; level: 'error' | 'warn' | 'info'; source: string; message: string; stack?: string }) => void): () => void


  // ─── PTY (per-tab) ────────────────────────────────────
  sendInput(tabId: string, data: string): void
  /** Stop: abort the in-flight upstream stream on the server + SIGINT the CLI. */
  interruptSession(tabId: string): void
  /** Submit a full user message via server-side paste + quiet-window Enter gate. */
  submitText(tabId: string, text: string): void
  resize(tabId: string, cols: number, rows: number): void
  onOutput(callback: (tabId: string, data: string) => void): () => void
  onExit(callback: (tabId: string, code: number) => void): () => void

  // KaminIDE host Monaco editor's active file + selection, for the composer's
  // "attach file" toggle. null when no text editor is active.
  onEditorSelection(callback: (sel: { path: string; startLine: number; endLine: number; text: string } | null) => void): () => void
  // Pull the current host-editor selection on mount (seeds before the first
  // onEditorSelection event fires).
  getEditorSelection(): Promise<{ path: string; startLine: number; endLine: number; text: string } | null>


  // ─── MCP permissions ──────────────────────────────────
  onPermissionRequest(callback: (tabId: string, req: PermissionRequest) => void): () => void
  respondPermission(requestId: string, decision: PermissionDecision): void

  // ─── MCP activity (per-tab) ───────────────────────────
  onMcpActivity(callback: (tabId: string, call: McpToolCall & { status: 'pending' | 'completed' | 'denied'; result?: unknown }) => void): () => void

  // ─── JSONL Viewer (per-tab) ──────────────────────────
  onJsonlEntries(cb: (tabId: string, entries: any[]) => void): () => void
  onJsonlStatus(cb: (tabId: string, status: { status: string; filePath?: string; error?: string }) => void): () => void
  onJsonlSubagentEntries(cb: (tabId: string, agentName: string, entries: any[], agentId?: string) => void): () => void

  // ─── Live MITM streaming (replaces by message.id) ────
  onStreamingEntry?(cb: (tabId: string, entry: any) => void): () => void
  /** Incremental text/thinking append between boundary snapshots (protocol>=1). */
  onStreamingDelta?(cb: (tabId: string, d: { msgId: string; blockIdx: number; appendText: string; kind: 'text' | 'thinking' }) => void): () => void
  /** Rate-limit / overload surfaced from the proxy while the CLI retries. */
  onStreamingStatus?(cb: (tabId: string, info: { code: number; retryAfterMs?: number }) => void): () => void

  // ─── JSONL Replay (after renderer reload) ────────────
  /** `tabId` replays just that session (a background tab refilling after memory
   *  eviction); omitted replays every open tab (renderer lost everything). */
  requestJsonlReplay(tabId?: string): void
  /** Report which tab the chat is actually SHOWING (the switch-sync gap signal). */
  reportBoundTab?(tabId: string): void
  /** Ask for the page of records just older than `beforePos` (the oldest byte
   *  offset the webview currently holds) — the scroll-up path for the windowed
   *  store. The answer arrives on `onOlderPage`. */
  loadOlderPage(tabId: string, beforePos: number, count: number): void
  /** The older page requested via loadOlderPage: records entirely older than
   *  `beforePos`, in file order — the renderer prepends them as-is. Empty
   *  `records` means the start of the transcript was reached. */
  onOlderPage(cb: (tabId: string, p: { beforePos: number; records: any[] }) => void): () => void
  /** Ask for the FULL compact-segment index (all segments incl. out-of-window +
   *  per-segment record counts), built from the disk mirror by timestamp. Answer
   *  on `onBoundaries`. */
  requestBoundaries(tabId: string): void
  onBoundaries(cb: (tabId: string, p: { boundaries: { ts: string }[]; counts: number[] }) => void): () => void
  /** Load one archived compact segment by TIMESTAMP range [fromTs, toTs) from the
   *  mirror (empty toTs = to newest). Answer on `onSegmentPage`; the renderer
   *  REPLACES its window with these records. */
  loadSegment(tabId: string, fromTs: string, toTs: string): void
  onSegmentPage(cb: (tabId: string, p: { fromTs: string; toTs: string; records: any[] }) => void): () => void
  /** Chunked-download progress for the session transcript. */
  onJsonlDownloadProgress(cb: (tabId: string, p: { received: number; total: number }) => void): () => void
  onJsonlDownloadAllProgress(cb: (p: {
    fileIndex: number; fileTotal: number; conversationId: string; bytes: number; size: number
  }) => void): () => void

  // ─── JSONL Download ───────────────────────────────────
  downloadJsonl(tabId: string): Promise<{ success: boolean; filePath?: string; error?: string }>
  /** Bulk export: pick a folder, then download EVERY transcript tied to the
   *  current token into it, each pulled in byte-range batches. Progress rides
   *  onJsonlDownloadAllProgress. */
  downloadAllJsonl(): Promise<{ success: boolean; dir?: string; count?: number; error?: string }>
  /** Save the session diagnostic JSON to a user-chosen file (was clipboard-only:
   *  a big session's dump is megabytes, which the clipboard handles badly and a
   *  bug report can't carry). */
  saveSessionDiag(fileName: string, content: string): Promise<{ success: boolean; filePath?: string; error?: string }>
  /** The user's standing instructions for the CURRENT token. Appended to (never
   *  replacing) the technical system prompt; applied to the next session. */
  getBasePrompt(): Promise<string>
  setBasePrompt(text: string): Promise<boolean>
  saveTextAs(text: string, defaultName?: string): Promise<{ ok: boolean; error?: string; filePath?: string }>

  // ─── Settings ─────────────────────────────────────────
  getConfig(): Promise<ConnectionConfig>
  setConfig(config: ConnectionConfig): void
  forceSync(): Promise<{ ok: boolean; error?: string; projectPath?: string | null }>
  getMachineId(): Promise<string>
  applyUpdate(): Promise<{ ok: boolean; error?: string }>
  quitAndInstallUpdate(): Promise<{ ok: boolean; error?: string }>
  onUpdateState(cb: (state: { state: 'downloading' | 'ready' | 'error'; error?: string }) => void): () => void

  // ─── Working directory ────────────────────────────────
  getCwd(): Promise<string | null>
  onCwdChanged(callback: (cwd: string) => void): () => void
  onConfigReady(callback: (hasConfig: boolean) => void): () => void

  // ─── Folder picker ──────────────────────────────
  selectFolder(): Promise<string | null>

  // ─── Clipboard image ─────────────────────────────────
  saveClipboardImage(base64Data: string, mimeType: string): Promise<string>
  openImageDialog(): Promise<Array<{ path: string; base64: string; mimeType: string }>>
  openFileDialog(): Promise<Array<{ path: string; name: string; ext: string; base64: string; mimeType: string; isImage: boolean }>>
  /** Не-image drop/paste: байты файла → temp с исходным именем → путь
   *  (замена getDroppedFilePath Electron-эпохи, аудит #70 A3). */
  saveDroppedFile(base64Data: string, name: string): Promise<string>
  readFileAsBase64(filePath: string): Promise<{ ok: boolean; base64?: string; mimeType?: string; error?: string }>

  // Hooks
  hooksList(): Promise<{ hooks: Record<string, Array<{ matcher?: string; hooks: Array<Record<string, unknown>> }>> }>
  hooksListAll(cwd?: string): Promise<{ hooks: Array<{ event: string; matcherIdx: number; matcher?: string; handler: Record<string, unknown>; source: { kind: string; pluginId?: string; manifestPath?: string; projectPath?: string } }> }>
  hooksGetLibrary(): Promise<{ templates: Array<{ id: string; title: string; description: string; event: string; matcher?: string; handler: Record<string, unknown>; rationale: string; category: string; recommendedHost?: string }> }>
  hooksGetLog(limit?: number): Promise<{ entries: Array<{ ts: number; sessionId: string; hookId: string; event: string; matcher?: string; source: { kind: string; pluginId?: string; projectPath?: string }; effectiveHost: string; outcome: string; exitCode: number; durationMs: number; stdoutPreview?: string; stderrPreview?: string }> }>
  hooksClearLog(): Promise<{ ok: boolean; error?: string }>
  hooksActive(sessionId: string): Promise<{ hooks: Array<Record<string, unknown>> }>
  hooksInstallTemplate(templateId: string): Promise<{ ok: boolean; error?: string }>
  hooksDelete(event: string, matcherIdx: number, handlerIdx: number): Promise<{ ok: boolean; error?: string }>
  hooksSave(draft: { event: string; matcher?: string; handler: Record<string, unknown>; index?: { matcherIdx: number; handlerIdx: number } }): Promise<{ ok: boolean; error?: string }>
  hooksTest(draft: { event: string; matcher?: string; handler: Record<string, unknown>; mockPayload?: Record<string, unknown> }): Promise<{ ok: boolean; error?: string; result?: { stdout: string; stderr: string; exitCode: number; outcome: string; durationMs: number }; effectiveHost?: string }>
  hooksGetPluginApproval(pluginId: string): Promise<{ approved: boolean; hashes: string[] }>
  hooksListPendingPluginApprovals(): Promise<Array<{ pluginId: string; hooks: Array<{ event: string; matcher?: string; handler: Record<string, unknown>; hash: string }>; approvedHashes: string[] }>>
  hooksSetPluginApproval(pluginId: string, hashes: string[]): Promise<{ ok: boolean; error?: string }>
  onPluginHooksAwaitingApproval(cb: (data: { pluginId: string; hooks: Array<{ event: string; matcher?: string; handler: Record<string, unknown>; hash: string }>; approvedHashes?: string[] }) => void): () => void
  openExternal(url: string): Promise<void>

  // ─── Chrome mic proxy (bypasses corporate endpoint protection on Windows) ──
  micStart(): Promise<boolean>
  micStop(): Promise<boolean>
  micConnected(): Promise<boolean>
  micLaunchChrome(): Promise<boolean>
  micToggleVisible(): Promise<boolean>
  onMicAudio(cb: (data: { base64: string; mimeType: string }) => void): () => void
  onMicLevels(cb: (bars: number[]) => void): () => void

  // ─── Voice input (Whisper) ──────────────────────────
  whisperTranscribe(audioBase64: string, mimeType: string): Promise<{ text?: string; error?: string }>
  checkMicAccess(): Promise<string>
  nativeMicStart(): Promise<{ error?: string }>
  nativeMicStop(): Promise<{ audioBase64?: string; error?: string }>

  // ─── App info ───────────────────────────────────────
  getVersion(): string

  // ─── Elicitation (interactive prompts from Claude) ──
  onElicitationRequest(callback: (tabId: string, req: ElicitationRequest) => void): () => void
  respondElicitation(requestId: string, action: 'accept' | 'deny' | 'dismiss', content?: Record<string, unknown>): void

  // ─── AskUserQuestion widget ───────────────────────
  onAskUserQuestion(callback: (tabId: string, data: { requestId: string; question: string; options?: string[]; plan?: string; isPlanApproval?: boolean }) => void): () => void
  respondAskUser(requestId: string, answer: string): void

  // ─── Session Title ─────────────────────────────────
  onSessionTitle(callback: (tabId: string, title: string) => void): () => void

  // ─── Session Tree ──────────────────────────────────
  onTreeUpdate(callback: (tree: TreeNode[]) => void): () => void
  onAgentSpawned(callback: (tabId: string, data: { parentSessionId: string; childSessionId: string; agentName: string; description: string }) => void): () => void
  requestTreeUpdate(): void
  onMcpLoading(callback: () => void): () => void
  onMcpReady(callback: () => void): () => void
  getMcpStatus(): Promise<boolean>

  // ─── Resume ────────────────────────────────────────
  resumeSession(config: ConnectionConfig & { conversationId: string }): Promise<string>
  getSavedSessions(): Promise<SavedSession[]>
  removeSavedSession(conversationId: string): Promise<void>
  deleteSessionData(conversationId: string): Promise<void>

  // ─── Permissions ────────────────────────────────────
  changePermissionMode(tabId: string, mode: string): void

  // ─── Effort ────────────────────────────────────────
  changeEffort(tabId: string, effort: string): void
  onSessionRestarted(callback: (tabId: string, data: { sessionId: string; effort?: string; model?: string }) => void): () => void
  // Fired when the user manually Reconnects a tab: the server ended the old
  // session and `--resume`s a BRAND-NEW PTY, so the console must wipe the dead
  // session's screen (broadcast → every iframe clears its own terminal).
  onConsoleReset(callback: (tabId: string) => void): () => void

  // ─── Model ────────────────────────────────────────
  changeModel(tabId: string, model: string): void

  // ─── Model ────────────────────────────────────────
  getTabModel(tabId: string): Promise<string | null>

  // ─── External MCP Servers ────────────────────────
  getMcpServers(): Promise<ExternalMcpServerInfo[]>
  addMcpServer(config: Omit<ExternalMcpServerConfig, 'id'>): Promise<ExternalMcpServerInfo>
  updateMcpServer(id: string, config: Partial<Omit<ExternalMcpServerConfig, 'id'>>): Promise<ExternalMcpServerInfo | null>
  removeMcpServer(id: string): Promise<void>
  toggleMcpServer(id: string, enabled: boolean): Promise<void>
  testMcpServer(id: string): Promise<{ ok: boolean; tools: string[]; error?: string }>
  getMcpTestLog(id: string): Promise<Array<{ ts: number; level: string; msg: string }>>
  clearMcpTestLog(id: string): Promise<void>
  onMcpTestLog(cb: (payload: { id: string; entry: { ts: number; level: string; msg: string } }) => void): () => void
  onMcpTestLogCleared(cb: (payload: { id: string }) => void): () => void
  oauthConnectMcpServer(id: string): Promise<{ ok: boolean; error?: string }>
  oauthDisconnectMcpServer(id: string): Promise<{ ok: boolean }>
  oauthMcpStatus(id: string): Promise<{ connected: boolean }>
  getMcpResources(id: string): Promise<McpResourceInfo[]>
  readMcpResource(id: string, uri: string): Promise<{ ok: boolean; result?: unknown; error?: string }>
  getMcpPrompts(id: string): Promise<McpPromptInfo[]>
  getMcpPrompt(id: string, name: string, args?: Record<string, unknown>): Promise<{ ok: boolean; result?: unknown; error?: string }>
  listMcpRegistry(): Promise<{ ok: boolean; servers: any[]; error?: string }>
  onMcpElicitation(callback: (req: { requestId: string; serverId: string; serverName: string; message?: string; requestedSchema?: unknown }) => void): () => void
  respondMcpElicitation(requestId: string, payload: unknown): void
  onMcpProgress(callback: (p: { serverId: string; serverName: string; progressToken: unknown; progress: number; total?: number; message?: string }) => void): () => void
  onMcpServersChanged(callback: (servers: ExternalMcpServerInfo[]) => void): () => void

  // ─── Skills ─────────────────────────────────────────
  listSkills(): Promise<{ name: string; fileName: string; description: string; path: string }[]>
  listAgents(): Promise<Array<{
    name: string; fileName: string; description: string; model: string | null; tools: string[]; path: string; source: string
    color?: string; effort?: string | number; maxTurns?: number; disallowedTools?: string[]
    memory?: string; isolation?: string; background?: boolean
  }>>
  listOutputStyles(): Promise<Array<{ name: string; description: string; path: string; source: string }>>
  createSkill(name: string, content: string): Promise<{ name: string; fileName: string; path: string }>
  deleteSkill(fileName: string): Promise<void>
  openSkill(filePath: string): Promise<void>
  readSkill(filePath: string): Promise<string | null>
  showSkillInFolder(filePath: string): Promise<void>

  // ─── Team messaging ──────────────────────────────────
  sendTeamMessage(teamName: string, agentName: string, message: string): Promise<void>

  // ─── Plugins ──────────────────────────────────────────
  listInstalledPlugins(): Promise<{ name: string; marketplace: string; version: string; scope: string; installPath: string; isCached: boolean }[]>
  getPluginContents(pluginDir: string): Promise<{
    skills: { name: string; description?: string }[]
    agents: { name: string; description?: string }[]
    commands: { name: string; description?: string }[]
    mcpServers: string[]
    hooks: { event: string; matcher?: string; type: string; summary: string }[]
  } | null>
  syncPluginCache(pluginName: string, marketplace: string): Promise<{ version: string; cacheDir: string }>
  getPluginSourcePath(pluginName: string, marketplace: string): Promise<string | null>
  openFolder(folderPath: string): void
  revealInExplorer(filePath: string): void

  // ─── Marketplace / Plugin Browser ────────────────────
  listMarketplaces(): Promise<MarketplaceInfo[]>
  browseMarketplace(marketplaceName: string): Promise<MarketplacePlugin[]>
  onPluginsBrowseProgress(cb: (p: { marketplace: string; done: number; total: number; current: string | null }) => void): () => void
  browseNestedMarketplace(pluginPath: string): Promise<MarketplacePlugin[]>
  installPlugin(pluginName: string, marketplace: string, pluginPath: string): Promise<void>
  installLocalPlugin(): Promise<{ name: string; marketplace: string; version: string; installPath: string } | null>
  uninstallPlugin(pluginName: string, marketplace: string): Promise<void>
  setPluginEnabled(pluginId: string, enabled: boolean): Promise<{ ok: boolean; restartRequired?: boolean; error?: string }>
  addMarketplace(name: string, gitUrl: string, auth?: { username?: string; token?: string } | null): Promise<MarketplaceInfo>
  setMarketplaceAuth(name: string, auth: { username?: string; token?: string } | null): Promise<{ ok: boolean; error?: string; warning?: string }>
  removeMarketplace(name: string): Promise<void>
  updateMarketplace(name: string): Promise<{ ok: boolean; lastUpdated?: string; error?: string; changed?: boolean }>
  getPluginOptionsSchema(pluginId: string): Promise<{ schema: Record<string, any>; values: Record<string, unknown>; sensitiveKeys: string[] }>
  savePluginOptions(pluginId: string, values: Record<string, unknown>): Promise<{ ok: boolean }>
  listMonitors(): Promise<Array<{ id: string; tabId: string; pluginId: string; pluginName: string; monitorName: string; description: string; command: string; status: 'running' | 'exited' | 'error'; startedAt: number; exitCode?: number | null }>>
  getMonitorLog(id: string): Promise<Array<{ ts: number; stream: 'stdout' | 'stderr' | 'meta'; text: string }>>
  onMonitorOutput(cb: (payload: { id: string; entry: { ts: number; stream: string; text: string } }) => void): () => void
  onMonitorStarted(cb: (payload: { id: string; tabId: string; pluginId: string; pluginName: string; monitorName: string; description: string; command: string }) => void): () => void
  onMonitorStopped(cb: (payload: { id: string }) => void): () => void
  onMonitorStatus(cb: (payload: { id: string; status: string; exitCode?: number | null }) => void): () => void
  listLspServers(): Promise<Array<{ id: string; pluginId: string; serverKey: string; extensions: string[]; initialized: boolean }>>
  restartLspServers(): Promise<{ ok: boolean }>
  refreshAllMarketplaces(): Promise<{ ok: boolean }>
  setMarketplaceAutoUpdate(name: string, autoUpdate: boolean): Promise<{ ok: boolean; error?: string }>
  onMarketplaceUpdated(cb: (payload: { name: string }) => void): () => void

  // ─── VS Code integration ──────────────────────────────────
  isVscodeAvailable(): Promise<boolean>
  openInVscode(cwd: string): void

  // ─── Session activity (OSC spinner) ──────────────────────
  onSessionActivity(callback: (tabId: string, data: { rawTitle: string; isWorking: boolean }) => void): () => void

  // ─── Window controls (custom titlebar) ──────────────────
  windowMinimize(): void
  windowMaximize(): void
  windowClose(): void
  windowToggleDevTools(): void
  windowIsMaximized(): boolean
  onWindowMaximized(callback: (maximized: boolean) => void): () => void

  // ─── FilePanel terminal (PTY) ────────────────────────
  shellsList(): Promise<Array<{ id: string; label: string; command: string; args: string[]; icon?: string }>>
  ptySpawn(opts: { shellId?: string; cwd: string; cols: number; rows: number }): Promise<{ ptyId: string; profile: { id: string; label: string; command: string; args: string[]; icon?: string } }>
  ptyWrite(ptyId: string, data: string): void
  ptyResize(ptyId: string, cols: number, rows: number): void
  ptyKill(ptyId: string): void
  onPtyData(callback: (e: { ptyId: string; data: string }) => void): () => void
  onPtyExit(callback: (e: { ptyId: string; exitCode: number; signal?: number }) => void): () => void

  // ─── FilePanel file viewer ───────────────────────────
  fileViewerRead(filePath: string): Promise<{ content: string; mtimeMs: number; size: number }>
  fileViewerReadBinary(filePath: string): Promise<{ base64: string; mtimeMs: number; size: number }>
  fileViewerWrite(filePath: string, content: string): Promise<{ mtimeMs: number; size: number }>
  fileViewerWatch(filePath: string): void
  fileViewerUnwatch(filePath: string): void
  onFileViewerExternalChange(callback: (e: { path: string; content: string; mtimeMs: number }) => void): () => void

  // ─── FilePanel project tree ──────────────────────────
  fileTreeListDir(dir: string): Promise<Array<{ name: string; path: string; isDir: boolean; mtimeMs: number; size: number }>>
  fileTreeRename(oldPath: string, newPath: string): Promise<{ mtimeMs: number; size: number }>
  fileTreeWatch(root: string): void
  fileTreeUnwatch(root: string): void
  onFileTreeChange(callback: (e: { root: string; changedDir: string; changedPath?: string }) => void): () => void

  // ─── Plugin source refresh (marketplace lifecycle) ───────
  refreshPluginSource(
    pluginName: string,
    marketplace: string,
  ): Promise<{ ok: boolean; changed?: boolean; version?: string; skipped?: string; error?: string; note?: string }>
  retryPluginSource(pluginName: string, marketplace: string): Promise<{ ok: boolean; error?: string }>

  /** Generic IPC pass-through used by a few panels that talk to dynamic
   *  channels (whisper, plugin auth modals). New code should add a typed
   *  method instead — this exists for parity with preload's open invoke. */
  invoke(channel: string, ...args: unknown[]): Promise<unknown>
}

/** @deprecated Use KaminBridgeApi. */
export type ElectronBridge = KaminBridgeApi
