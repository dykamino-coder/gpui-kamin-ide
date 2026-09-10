import path from 'path'
import fs from 'fs'
import os from 'os'
import { randomUUID } from 'crypto'
import type { BrowserWindow } from '@kaminide/host-compat'
import { ConnectionManager } from './ws/connection-manager'
import { syncUserData, syncProjectData, resetSyncTimers } from './sync/sync-client'
import type { ConnectionConfig, ConnectionState, TabInfo, PermissionDecision } from '../shared/types'

const _tabLogPath = path.join(os.tmpdir(), 'ocb-tab-debug.log')
function tabLog(msg: string) {
  try { fs.appendFileSync(_tabLogPath, `[${new Date().toISOString()}] ${msg}\n`) } catch {}
}

export interface Tab {
  id: string
  config: ConnectionConfig
  connection: ConnectionManager
  label: string           // "#1", "#2", etc.
  folderName: string      // basename(cwd) for sidebar grouping
  createdAt: string       // ISO timestamp
  effort?: string         // current effort level
  model?: string          // current model
  sessionTitle?: string   // auto-generated topic from CLI ("Fix auth bug")
  conversationId?: string // CLI conversation UUID (for resume)
  settingsDir?: string    // server-side settings directory path
}

export class TabManager {
  private tabs = new Map<string, Tab>()
  private activeTabId: string | null = null
  // Monotonic switch sequence. Every `tab:switched` carries the seq at which it
  // was emitted; the webview keeps the highest it has seen and rejects any lower
  // one. This is the whole fix for the overtake bug — activeTabId was pure
  // last-write-wins across a fork-IPC boundary, so a late switch to a session
  // the user had already left could win and render the wrong conversation
  // ("went back to A, the iframe finished loading B"). Assigned here because the
  // extension consumes host activeChange events in order, so seq order == intent
  // order regardless of how the IPC frames race downstream.
  private switchSeq = 0
  /** Emit `tab:switched` with the next sequence number. The single choke point
   *  for every switch so none can slip out unsequenced. */
  private emitSwitched(tabId: string | null): void {
    this.window.webContents.send('tab:switched', tabId, ++this.switchSeq)
  }
  private window: BrowserWindow

  // Per-folder session counter: folderName → next number
  private folderCounters = new Map<string, number>()

  /** Invoked when a tab's cwd becomes active (tab created with cwd). */
  onProjectOpened: ((cwd: string) => void) | null = null
  /** Invoked when a tab with a cwd is closed and no other tab references that cwd. */
  onProjectClosed: ((cwd: string) => void) | null = null
  /** Fires just before a tab is deleted, so the caller can persist a saved
   *  session record. Receiver may inspect tab.conversationId, sessionTitle,
   *  model, etc. */
  onTabClosing: ((tab: Tab) => void) | null = null
  /** Fires right after a tab is created and registered. Used to start
   *  per-tab background work — plugin monitors, etc. — that needs to die
   *  when the tab closes. */
  onTabCreated: ((tab: Tab) => void) | null = null

  constructor(window: BrowserWindow) {
    this.window = window
  }

  /** Snapshot of every distinct cwd currently referenced by an open tab. */
  getDistinctCwds(): string[] {
    const set = new Set<string>()
    for (const tab of this.tabs.values()) {
      if (tab.config.cwd) set.add(tab.config.cwd)
    }
    return [...set]
  }

  /**
   * Create a new tab with its own ConnectionManager.
   * Automatically connects and becomes the active tab.
   */
  createTab(config: ConnectionConfig): string {
    const tabId = randomUUID()
    const folderName = config.cwd ? path.basename(config.cwd) : 'default'
    tabLog(`createTab: tabId=${tabId.slice(0,8)} folder=${folderName} cwd=${config.cwd || 'none'} total=${this.tabs.size + 1} stack=${new Error().stack?.split('\n').slice(1, 4).map(l => l.trim()).join(' | ')}`)

    // Increment per-folder counter
    const counter = (this.folderCounters.get(folderName) ?? 0) + 1
    this.folderCounters.set(folderName, counter)
    const label = `#${counter}`

    const connection = new ConnectionManager(config, this.window, tabId)

    const tab: Tab = {
      id: tabId,
      config,
      connection,
      label,
      folderName,
      createdAt: new Date().toISOString(),
    }

    this.tabs.set(tabId, tab)
    this.activeTabId = tabId

    // Notify renderer
    this.window.webContents.send('tab:created', this.getTabInfo(tab))
    this.emitSwitched(tabId)
    this.notifyTabListChanged()

    // Fire after renderer is notified so the listener can already query
    // anything that depends on the tab being registered.
    try { this.onTabCreated?.(tab) } catch (err) { tabLog(`onTabCreated threw: ${err}`) }

    // Emit project-opened only when this cwd becomes newly referenced.
    if (config.cwd) {
      const otherRefs = [...this.tabs.values()].filter(t => t.id !== tabId && t.config.cwd === config.cwd).length
      if (otherRefs === 0) this.onProjectOpened?.(config.cwd)
    }

    // Sync user & project data to bridge before connecting (fire-and-forget).
    // Guard: the sync takes seconds — if the tab was closed meanwhile (user
    // closed it, or a token swap ran closeAllTabs), connect() would spawn an
    // orphan CLI session for a ghost tab that nothing ever closes until the
    // server's 30-min idle reaper.
    const connectIfAlive = () => { if (this.tabs.has(tabId)) connection.connect() }
    this.triggerSync(config).then(connectIfAlive).catch(connectIfAlive)

    return tabId
  }

  /**
   * Disconnect a tab's WebSocket session without removing the tab.
   * Keeps chat history visible; user can reconnect or close separately.
   */
  disconnectTab(tabId: string): void {
    const tab = this.tabs.get(tabId)
    if (!tab) return
    // Explicit user action — end the server session now (a bare socket close
    // would only detach it for the reattach grace window).
    tab.connection.disconnect({ endSession: true })
  }

  /**
   * Re-open a disconnected tab's WebSocket session (ConnectionManager.connect
   * tears down any stale socket first). Reuses the tab's stored config so the
   * server resumes the same conversation.
   */
  reconnectTab(tabId: string): void {
    const tab = this.tabs.get(tabId)
    if (!tab) return
    // The prior disconnectTab ended the server session (endSession), so this
    // connect() resumes a BRAND-NEW PTY. Tell every webview to wipe the dead
    // session's console screen — the terminal lives in the Console tool's iframe
    // (a different JS context from the Reconnect button), so this must be a
    // broadcast, not an in-webview call. Only the MANUAL reconnect path fires
    // this; auto-reconnect (ConnectionManager.scheduleReconnect → connect) does
    // NOT, so a transient blip that reattaches the live PTY keeps its scrollback.
    this.window.webContents.send('bridge:console-reset', tabId)
    tab.connection.connect()
  }

  /**
   * Close a tab and disconnect its session.
   */
  /** Close every open tab. Used when the user swaps tokens — the in-memory
   *  tabs belong to the old token's server-side sessions and can't be reused. */
  closeAllTabs(): void {
    const ids = Array.from(this.tabs.keys())
    for (const id of ids) this.closeTab(id)
  }

  closeTab(tabId: string): void {
    const tab = this.tabs.get(tabId)
    if (!tab) return

    // Persist the session BEFORE deletion so closing a tab mid-dialog moves
    // it to "inactive" instead of silently dropping it. `configStore.addSavedSession`
    // dedups by conversationId — if a save already happened via
    // onConversationId, this is a no-op upsert with a fresher lastActivity.
    if (tab.conversationId && this.onTabClosing) {
      this.onTabClosing(tab)
    }

    // Explicit close — end the server session (see disconnectTab).
    tab.connection.disconnect({ endSession: true })
    const closedCwd = tab.config.cwd
    this.tabs.delete(tabId)

    // If we closed the active tab, switch to another or null
    if (this.activeTabId === tabId) {
      const remaining = Array.from(this.tabs.keys())
      this.activeTabId = remaining.length > 0 ? remaining[remaining.length - 1] : null
      if (this.activeTabId) {
        this.emitSwitched(this.activeTabId)
      }
    }

    this.window.webContents.send('tab:closed', tabId)
    this.notifyTabListChanged()

    // Emit project-closed only when no other tab still references this cwd.
    if (closedCwd) {
      const stillOpen = [...this.tabs.values()].some(t => t.config.cwd === closedCwd)
      if (!stillOpen) this.onProjectClosed?.(closedCwd)
    }
  }

  /**
   * Switch the active tab (for the renderer to display).
   */
  switchTab(tabId: string): void {
    const tab = this.tabs.get(tabId)
    if (!tab) return
    tabLog(`switchTab: tabId=${tabId.slice(0, 8)} seq=${this.switchSeq + 1}`)
    // First reveal of a warm-pool tab: turn its live forwarding on. The webview
    // finds its store empty and pulls the cached transcript itself.
    tab.connection.setRendererMuted(false)
    // Lazy attach: warm-pool tabs are created WITHOUT a server connection (see
    // resumeSession background) — the first switch binds to the live CLI.
    if (tab.connection.getExtendedState().status === 'disconnected') {
      tab.connection.connect()
    }
    this.activeTabId = tabId
    this.emitSwitched(tabId)
  }

  /**
   * Get info for all tabs.
   */
  listTabs(): TabInfo[] {
    return Array.from(this.tabs.values()).map(tab => this.getTabInfo(tab))
  }

  /**
   * Find an existing tab with the given cwd (case-insensitive on Windows).
   */
  findTabByCwd(cwd: string): string | null {
    const normalized = cwd.replace(/[\\/]+$/, '').toLowerCase()
    for (const tab of this.tabs.values()) {
      const tabCwd = (tab.config.cwd ?? '').replace(/[\\/]+$/, '').toLowerCase()
      if (tabCwd === normalized) return tab.id
    }
    return null
  }

  /**
   * Get the active tab ID.
   */
  getActiveTabId(): string | null {
    return this.activeTabId
  }

  /**
   * Get the ConnectionManager for a specific tab.
   */
  getConnection(tabId: string): ConnectionManager | undefined {
    return this.tabs.get(tabId)?.connection
  }

  /**
   * Get the active tab's ConnectionManager.
   */
  getActiveConnection(): ConnectionManager | undefined {
    if (!this.activeTabId) return undefined
    return this.tabs.get(this.activeTabId)?.connection
  }

  /**
   * Get connection state for a tab.
   */
  getConnectionState(tabId: string): ConnectionState {
    const tab = this.tabs.get(tabId)
    return tab?.connection.getState() ?? { status: 'disconnected', authority: 'missing', authorityGeneration: 0, authoritySequence: 0, revision: 0 }
  }

  /**
   * Send PTY input to a specific tab.
   */
  interruptSession(tabId: string): void {
    this.tabs.get(tabId)?.connection.interruptSession()
  }

  sendInput(tabId: string, data: string): void {
    this.tabs.get(tabId)?.connection.sendInput(data)
  }

  /** Submit a user message via the server-side quiet-window Enter gate. */
  submitText(tabId: string, text: string): void {
    this.tabs.get(tabId)?.connection.submitText(text)
  }

  /**
   * Resize terminal for a specific tab.
   */
  resize(tabId: string, cols: number, rows: number): void {
    this.tabs.get(tabId)?.connection.resize(cols, rows)
  }

  /**
   * Respond to permission request (routed to correct tab's connection).
   */
  respondPermission(requestId: string, decision: PermissionDecision): void {
    // Permission requests may come from any tab, try all
    for (const tab of this.tabs.values()) {
      tab.connection.respondPermission(requestId, decision)
    }
  }

  /**
   * Respond to MCP tool call — ТОЛЬКО владельцу requestId. Броадкаст во все
   * табы слал mcp:response чужим сессиям (сервер получал ответы на
   * неизвестные requestId) и плодил ложные mcp-activity (аудит #70 C15).
   */
  respondMcp(requestId: string, result: unknown): void {
    for (const tab of this.tabs.values()) {
      if (tab.connection.hasPendingMcp(requestId)) {
        tab.connection.respondMcp(requestId, result)
        return
      }
    }
    console.warn(`[mcp] respondMcp: no tab owns request ${requestId}`)
  }

  /**
   * Deny MCP tool call — ТОЛЬКО владельцу requestId (см. respondMcp).
   */
  denyMcp(requestId: string, reason: string): void {
    for (const tab of this.tabs.values()) {
      if (tab.connection.hasPendingMcp(requestId)) {
        tab.connection.denyMcp(requestId, reason)
        return
      }
    }
    console.warn(`[mcp] denyMcp: no tab owns request ${requestId}`)
  }

  /**
   * Respond to elicitation request (routed to correct tab's connection).
   */
  respondElicitation(requestId: string, action: 'accept' | 'deny' | 'dismiss', content?: Record<string, unknown>): void {
    for (const tab of this.tabs.values()) {
      tab.connection.respondElicitation(requestId, action, content)
    }
  }

  /**
   * Resume a previous session by creating a new tab with resume flag.
   *
   * `background: true` = the warm pool: the tab is created, connected and its
   * transcript cached WITHOUT becoming the active tab (no `tab:switched`, no
   * focus steal) — so a later switch to this session renders instantly instead
   * of paying the cold connect+replay in front of the user. Background resumes
   * also skip the per-tab sync: the foreground session's sync already pushed
   * the user data, and a burst of warm-pool resumes must not become a sync storm.
   */
  resumeSession(config: ConnectionConfig & { conversationId: string; sessionLabel?: string }, opts: { background?: boolean } = {}): string {
    const tabId = randomUUID()
    const folderName = config.cwd ? path.basename(config.cwd) : 'default'
    tabLog(`resumeSession: tabId=${tabId.slice(0,8)} convId=${config.conversationId.slice(0,8)} folder=${folderName} label=${config.sessionLabel || 'none'} bg=${opts.background ? 1 : 0} total=${this.tabs.size + 1}`)

    const counter = (this.folderCounters.get(folderName) ?? 0) + 1
    this.folderCounters.set(folderName, counter)
    const label = `#${counter}`

    const connection = new ConnectionManager(config, this.window, tabId, config.conversationId)

    const tab: Tab = {
      id: tabId,
      config,
      connection,
      label,
      folderName,
      createdAt: new Date().toISOString(),
      conversationId: config.conversationId,
      sessionTitle: config.sessionLabel,
    }

    this.tabs.set(tabId, tab)
    if (!opts.background) this.activeTabId = tabId

    this.window.webContents.send('tab:created', this.getTabInfo(tab))
    if (!opts.background) this.emitSwitched(tabId)
    this.notifyTabListChanged()

    if (opts.background) {
      // Warm DATA only: no server connection at boot. Connecting the whole
      // pool replayed several histories into the one exthost thread at once
      // and starved the webview's startup invokes for tens of seconds. The
      // transcript paints from the local mirror (prefill / activation-time
      // replay); the WS+CLI attach lazily on first switch — the server keeps
      // the CLI alive anyway, so that attach is a fast rebind, not a respawn.
      connection.setRendererMuted(true)
      return tabId
    }

    // Sync user & project data to bridge before connecting (fire-and-forget).
    // Тот же guard, что в createTab: закрытый за время sync таб не должен
    // порождать orphan-сессию (WS open handler шлёт session:resume сам —
    // conversationId передан конструктору).
    const connectIfAlive = () => { if (this.tabs.has(tabId)) connection.connect() }
    this.triggerSync(config).then(connectIfAlive).catch(connectIfAlive)

    return tabId
  }

  /**
   * Disconnect all tabs. Called on app quit / window close.
   */
  disconnectAll(): void {
    for (const tab of this.tabs.values()) {
      tab.connection.disconnect()
    }
    this.tabs.clear()
    this.activeTabId = null
  }

  /**
   * Number of open tabs.
   */
  get size(): number {
    return this.tabs.size
  }

  /**
   * Get mapping from server sessionId → tab info.
   * Used to enrich session tree with tab metadata (label, folderName).
   */
  getSessionTabMap(): Map<string, TabInfo> {
    const map = new Map<string, TabInfo>()
    for (const tab of this.tabs.values()) {
      const sessionId = tab.connection.getSessionId()
      if (sessionId) {
        map.set(sessionId, this.getTabInfo(tab))
      }
    }
    return map
  }

  /** Get the first active connection (for sending commands to server) */
  getFirstConnection(): ConnectionManager | null {
    for (const tab of this.tabs.values()) {
      if (tab.connection.getExtendedState().status === 'authenticated') return tab.connection
    }
    // Fallback: any connection
    const first = this.tabs.values().next()
    return first.done ? null : first.value.connection
  }

  // ─── Private ────────────────────────────────────────────

  /**
   * Sync user-level and project-level data to the bridge server.
   * Called before WS connect; debounced internally (60s).
   */
  private async triggerSync(config: ConnectionConfig): Promise<void> {
    if (!config.serverUrl || !config.token) return
    try {
      // Every new chat resets the debounce so skills/agents/commands the user
      // just added in the UI hit the server before the session actually starts.
      resetSyncTimers()
      // Sync user data (skills, agents, CLAUDE.md, settings.json)
      await syncUserData(config.serverUrl, config.token)
      // Sync project data if CWD is set
      if (config.cwd) {
        await syncProjectData(config.serverUrl, config.token, config.cwd)
      }
    } catch (err) {
      tabLog(`triggerSync error: ${err instanceof Error ? err.message : String(err)}`)
    }
  }

  /**
   * Update effort/model for a tab (called from session-restarted IPC).
   */
  updateTabSessionInfo(tabId: string, effort?: string, model?: string): void {
    const tab = this.tabs.get(tabId)
    if (!tab) return
    if (effort) tab.effort = effort
    if (model) tab.model = model
    // Pick up settingsDir from connection (set during session:created)
    const sd = tab.connection.getSettingsDir()
    if (sd) tab.settingsDir = sd
    this.notifyTabListChanged()
  }

  /**
   * Update session title (auto-generated topic from CLI).
   */
  updateTabSessionTitle(tabId: string, title: string): void {
    const tab = this.tabs.get(tabId)
    if (!tab) return
    tab.sessionTitle = title
    this.notifyTabListChanged()
  }

  getTabSessionTitle(tabId: string): string | undefined {
    return this.tabs.get(tabId)?.sessionTitle
  }

  getTab(tabId: string): Tab | undefined {
    return this.tabs.get(tabId)
  }

  private getTabInfo(tab: Tab): TabInfo {
    // One atomic read: status/sessionId/retry metadata must describe the same
    // connection revision or a list snapshot can manufacture a hybrid state.
    const connection = tab.connection.getState()
    return {
      id: tab.id,
      cwd: tab.config.cwd ?? '',
      label: tab.label,
      folderName: tab.folderName,
      createdAt: tab.createdAt,
      status: connection.status,
      connectionAuthority: connection.authority,
      connectionAuthorityGeneration: connection.authorityGeneration,
      connectionAuthoritySequence: connection.authoritySequence,
      connectionRevision: connection.revision,
      error: connection.error,
      nextRetryAt: connection.nextRetryAt,
      retryAttempt: connection.retryAttempt,
      effort: tab.effort,
      model: tab.model,
      sessionTitle: tab.sessionTitle,
      conversationId: tab.conversationId,
      sessionId: connection.sessionId,
      settingsDir: tab.settingsDir,
    }
  }

  private notifyTabListChanged(): void {
    if (this.window.isDestroyed()) return
    this.window.webContents.send('tab:list-changed', this.listTabs())
  }

  /** Public version for external callers (e.g. onConversationId, onSessionInfo) */
  notifyTabListChangedPublic(): void {
    this.notifyTabListChanged()
  }
}
