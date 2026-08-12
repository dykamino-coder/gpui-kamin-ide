// External MCP Server Manager
// Manages connections to external MCP servers (HTTP, SSE, WS, stdio).
// Auto-discovers local Claude MCP configs, discovers tools, forwards tool calls.
//
// Transport-specific wire logic lives in ./transports/*.ts.
// Auto-discovery (plugins / ~/.claude.json / project .mcp.json) lives in ./discovery.ts.
// Test-log ring buffer helpers live in ./test-log.ts.

import { randomUUID } from 'crypto'
import path from 'path'
import fs from 'fs'
import os from 'os'
import type { BrowserWindow } from '@kaminide/host-compat'
import type {
  ExternalMcpServerConfig,
  ExternalMcpServerInfo,
  McpResourceInfo,
  McpResourceTemplateInfo,
  McpPromptInfo,
} from '../../shared/types'
import {
  getMcpStore as getStore,
  configToClaudeJsonEntry,
  mutateClaudeJson,
  loadToolSchemaCache,
  saveToolSchemaCache,
  type CachedToolSchema,
} from './state-persistence'
import type { McpResult } from './tool-registry'
import {
  type OAuthServerMetadata,
} from './oauth-flow'
import { loadTokens, deleteTokens, migrateLegacyStore, type StoredTokens } from './oauth-store'
import * as oauthBridge from './oauth-bridge'
import { teardownServer, dispatchRpcRequest } from './connection-lifecycle'
import { handleServerMessage as handleServerMessageImpl, type MessageHandlerCtx } from './server-message-handlers'
import {
  discoverPluginConfigs,
  discoverProjectConfigs,
  discoverFromClaudeJsonAsync,
  resolveEnvVars,
} from './discovery'
import {
  appendLog as appendLogShared,
  clearTestLog as clearTestLogShared,
  getTestLog as getTestLogShared,
  type McpTestLogEntry,
} from './test-log'
import { connectStdio, callStdioTool, stdioJsonRpc } from './transports/stdio'
import { connectHttp, callHttpTool, httpJsonRpcAuth } from './transports/http'
import { connectSseLegacy, sseJsonRpcRequest } from './transports/sse-legacy'
import { connectWs, callWsTool, wsJsonRpc } from './transports/ws'
import type { McpServerState, TransportContext } from './transports/context'
import { collectMcpList } from './pagination'

export type { McpTestLogEntry } from './test-log'

/** Tool schema returned by getExternalToolSchemas() */
export interface ExternalToolSchema {
  name: string
  description: string
  inputSchema: Record<string, unknown>
  serverName: string
}

export interface ExternalContentCatalog {
  resources: Array<{ uri: string; name: string; description?: string; mimeType?: string; serverId: string; rawUri: string }>
  resourceTemplates: Array<{ uriTemplate: string; name: string; description?: string; mimeType?: string; serverId: string; rawUriTemplate: string }>
  prompts: Array<{ name: string; description?: string; arguments?: unknown[]; serverId: string; rawName: string }>
}

/** Auto-reconnect budget for a flapping/failing server. Module-scoped so the
 *  discovery re-scan can preserve a server that has already spent it (see
 *  discoverLocalMcpConfigs) — otherwise the delete+re-add on every re-scan hands
 *  a permanently-broken server a fresh budget and it never gives up. */
const MAX_RECONNECT_ATTEMPTS = 10

/** Identity of a server's CONNECTION config — everything that decides how/where
 *  it connects. Used to tell "same broken server, re-scanned" (carry the spent
 *  reconnect budget) from "the user edited it" (fresh budget, retry). Excludes
 *  cosmetic/source fields (name, sourcePath, enabled) that don't change wiring. */
function mcpConnectionFingerprint(c: ExternalMcpServerConfig): string {
  return JSON.stringify([c.type, c.command ?? null, c.args ?? null, c.url ?? null,
    c.env ?? null, c.workingDirectory ?? null, c.headers ?? null])
}

/** Ленивые stdio-серверы (экономия RAM): каждый stdio-MCP — child-процесс
 *  (замер perf-ram: unity-relay один держал 445MB private, playwright/
 *  chrome-devtools/context7 по ~140MB — ~950MB на idle-буте). Схемы тулов
 *  кэшируются на диск при первом успешном коннекте; со второго запуска CLI
 *  получает тулы ИЗ КЭША, а процесс поднимается только при первом tools/call.
 *  Сервер без кэша (новый/правленый конфиг) коннектится сразу — иначе его
 *  тулы не появились бы вовсе. KAMIN_EAGER_EXTERNAL_MCP=1 — старое поведение. */
const LAZY_STDIO_MCP = process.env.KAMIN_EAGER_EXTERNAL_MCP !== '1'

function isStdio(c: ExternalMcpServerConfig): boolean {
  return c.type !== 'http' && c.type !== 'sse' && c.type !== 'ws'
}

function toolScopePart(value: string): string {
  return value.trim().replace(/[^a-zA-Z0-9_-]/g, '_') || 'server'
}

function exposedToolName(state: McpServerState, rawToolName: string): string {
  const c = state.config
  const server = toolScopePart(c.pluginServerName ?? c.name)
  if (c.sourceType === 'plugin' && c.pluginName) {
    return `plugin_${toolScopePart(c.pluginName)}_${server}__${rawToolName}`
  }
  // Preserve the bridge's long-standing public name for user/project MCP
  // servers. Only plugin tools require namespacing to match Claude's plugin
  // contract and avoid collisions between marketplaces.
  return rawToolName
}

export class McpServerManager {
  /** @internal */
  servers = new Map<string, McpServerState>()
  /** @internal */
  window: BrowserWindow
  /** Project roots to scan for `<cwd>/.mcp.json` — injected by the app. */
  private projectRoots = new Set<string>()
  /** Callback invoked whenever the set of connected tools changes */
  onToolsChanged: (() => void) | null = null

  /** Диск-кэш схем тулов stdio-серверов (fingerprint → схемы) для ленивого
   *  старта — см. LAZY_STDIO_MCP. Обновляется на каждом notifyChanged из
   *  живых toolSchemas и переживает рестарт IDE. */
  private lazySchemaCache: Record<string, CachedToolSchema[]> = loadToolSchemaCache()

  /** Идущие подключения по id сервера — дедуп для connectServer. */
  private connectInFlight = new Map<string, Promise<void>>()

  // Coalesce `mcp-servers-changed` broadcasts. 20 call sites fire this, each
  // sending the FULL server list; startup (N servers connecting) and a broken
  // server flapping (stdio/ws error → reconnect) otherwise emit a burst that
  // saturates the renderer main thread (the prod freeze amplifier). A leading +
  // trailing throttle collapses a storm into ≤1 broadcast per interval; the
  // trailing fire reads getServers() live so it always ships the latest state.
  private notifyTimer: ReturnType<typeof setTimeout> | null = null
  private notifyLastAt = 0
  private static readonly NOTIFY_MIN_INTERVAL_MS = 200

  constructor(window: BrowserWindow) {
    this.window = window
    // One-shot migration: previously we stored manually-added servers in
    // electron-store (isolated from Claude Code). Import any legacy entries
    // into `~/.claude.json` so both tools share config, then clear the store.
    const legacy = (getStore().get('mcpServers') ?? []).filter(c => !c.autoDiscovered)
    if (legacy.length > 0) {
      this.saveChain = mutateClaudeJson((mcpServers) => {
        for (const c of legacy) {
          if (!mcpServers[c.name]) {
            mcpServers[c.name] = configToClaudeJsonEntry(c)
          }
        }
      }).then(() => {
        getStore().set('mcpServers', [])
        console.log(`[MCP] Migrated ${legacy.length} legacy manual server(s) to ~/.claude.json`)
      }).catch(err => console.error('[MCP] Migration failed:', err))
    }
    // Configs themselves are loaded by discoverFromClaudeJsonAsync on startup —
    // nothing to seed here.
  }

  /** Build a fresh TransportContext snapshot. We rebuild per call rather than
   *  caching because `onToolsChanged` may be reassigned by the caller after
   *  construction. */
  private ctx(): TransportContext {
    const self = this
    return {
      window: this.window,
      servers: this.servers,
      appendLog: (id, level, msg) => self.appendLog(id, level, msg),
      notifyChanged: () => self.notifyChanged(),
      get onToolsChanged() { return self.onToolsChanged },
      handleServerMessage: (serverId, msg, sendResponse) => self.handleServerMessage(serverId, msg, sendResponse),
      buildHttpHeaders: (id) => self.buildHttpHeaders(id),
      tryRefreshTokens: (id) => self.tryRefreshTokens(id),
      saveConfigs: () => self.saveConfigs(),
      setDiscoveredOAuthMetadata: (id, asUrl, scopes) => self.setDiscoveredOAuthMetadata(id, asUrl, scopes),
      onUnexpectedDisconnect: (id, reason) => self.scheduleReconnect(id, reason),
    }
  }

  /** Auto-recovery: schedule a reconnect with exponential backoff after
   *  an unexpected disconnect (process crash, socket abort). Capped at
   *  10 attempts (~ < 20 minutes total) so a permanently-broken server
   *  doesn't churn forever. Manager.disconnectServer cancels the timer
   *  + clears the attempt counter so toggling "enabled" off-then-on
   *  starts fresh. */
  private scheduleReconnect(id: string, reason: string): void {
    const state = this.servers.get(id)
    if (!state) return
    if (state.expectedDisconnect) {
      state.expectedDisconnect = false
      return
    }
    if (!state.config.enabled) return
    // Refill the budget only for a connection that PROVED itself by staying up.
    // A crash after an hour of healthy service deserves a full retry budget; a
    // server that dies seconds after connecting is failing, and treating that as
    // success is what made the loop below immortal.
    const STABLE_CONNECTION_MS = 60_000
    const uptime = state.connectedAt !== undefined ? Date.now() - state.connectedAt : 0
    if (uptime >= STABLE_CONNECTION_MS) state.reconnectAttempts = 0
    state.connectedAt = undefined
    const attempts = state.reconnectAttempts ?? 0
    if (attempts >= MAX_RECONNECT_ATTEMPTS) {
      this.appendLog(id, 'error', `[reconnect] giving up after ${MAX_RECONNECT_ATTEMPTS} attempts — toggle the server off/on to retry`)
      state.status = 'error'
      state.error = `Auto-reconnect gave up after ${MAX_RECONNECT_ATTEMPTS} attempts. Last reason: ${reason}`
      this.notifyChanged()
      return
    }
    // 2s → 4s → 8s → 16s → 32s → 60s (cap). Cumulative ~5 min before hitting cap.
    const delayMs = Math.min(2000 * Math.pow(2, attempts), 60000)
    state.reconnectAttempts = attempts + 1
    this.appendLog(id, 'warn', `[reconnect] scheduled in ${Math.round(delayMs / 1000)}s (attempt ${attempts + 1}/${MAX_RECONNECT_ATTEMPTS}) — reason: ${reason}`)
    if (state.reconnectTimer) clearTimeout(state.reconnectTimer)
    state.reconnectTimer = setTimeout(() => {
      state.reconnectTimer = undefined
      const cur = this.servers.get(id)
      if (!cur || !cur.config.enabled || cur.status === 'connected' || cur.status === 'connecting') return
      this.appendLog(id, 'info', `[reconnect] attempting (attempt ${attempts + 1}/${MAX_RECONNECT_ATTEMPTS})`)
      this.connectServer(id).catch(() => { /* failure path schedules next attempt via exit handler */ })
    }, delayMs)
  }

  /** Serialises discovery runs so concurrent addProjectRoot/removeProjectRoot
   *  calls don't trample each other's `projectRoots` snapshot mid-scan. */
  private discoveryQueue: Promise<unknown> = Promise.resolve()
  private enqueueDiscovery<T>(fn: () => Promise<T>): Promise<T> {
    const next = this.discoveryQueue.then(fn, fn)
    // Keep the chain alive even if a run rejects — subsequent runs still go.
    this.discoveryQueue = next.catch(() => {})
    return next
  }

  /** Register a project root (tab cwd). Triggers re-discovery to pick up that project's .mcp.json. */
  addProjectRoot(cwd: string): void {
    if (!cwd) return
    if (this.projectRoots.has(cwd)) return
    this.projectRoots.add(cwd)
    // Re-run discovery so the new project's .mcp.json is picked up immediately.
    this.enqueueDiscovery(async () => {
      await this.discoverLocalMcpConfigs()
      for (const [id, state] of this.servers) {
        if (state.config.enabled && state.status === 'disconnected') {
          this.connectServer(id).catch(() => {})
        }
      }
    }).catch(() => {})
  }

  /** Unregister a project root (tab closed). Removes its auto-discovered servers. */
  removeProjectRoot(cwd: string): void {
    if (!this.projectRoots.delete(cwd)) return
    this.enqueueDiscovery(() => this.discoverLocalMcpConfigs()).catch(() => {})
  }

  /** Initialize: discover local MCP configs, then connect all enabled servers */
  async initAll(): Promise<void> {
    // Wait for any pending ~/.claude.json writes (e.g. the constructor's
    // one-shot migration from the legacy electron-store) to land on disk,
    // otherwise discovery below races the write and misses migrated entries.
    await this.saveChain
    // Discover local MCP configs from Claude settings
    await this.discoverLocalMcpConfigs()

    // One-shot migration of OAuth tokens from the pre-unified encrypted store
    // (`<userData>/mcp-oauth-tokens.json`) into `~/.claude/.credentials.json`.
    // Match legacy entries to in-memory servers by authServerUrl since the
    // legacy key (internal UUID) is regenerated on every discovery run.
    try {
      const moved = migrateLegacyStore((tokens) => {
        if (!tokens.authServerUrl) return null
        for (const state of this.servers.values()) {
          if (state.config.oauth?.authServerUrl === tokens.authServerUrl && state.config.url) {
            return {
              serverName: state.config.name,
              serverUrl: state.config.url,
              authServerUrl: state.config.oauth.authServerUrl,
            }
          }
        }
        return null
      })
      if (moved > 0) console.log(`[MCP OAuth] Migrated ${moved} token(s) from legacy store → ~/.claude/.credentials.json`)
    } catch (err) {
      console.error('[MCP OAuth] Legacy token migration failed:', err)
    }

    // Connect all enabled servers (fire and forget — individual failures are logged).
    // Skip any that already SPENT their reconnect budget on an unchanged config:
    // initAll is also the reload path (plugin install / data sync re-runs it), so
    // without this a permanently-broken server would be re-attempted on every sync,
    // re-arming the exact churn the give-up cap exists to stop. At startup nothing
    // has spent its budget, so every enabled server is still tried.
    const promises: Promise<void>[] = []
    for (const [id, state] of this.servers) {
      if (state.config.enabled && (state.reconnectAttempts ?? 0) < MAX_RECONNECT_ATTEMPTS) {
        // Ленивый stdio: схемы уже в диск-кэше → процесс НЕ поднимаем, тулы
        // отдаются из кэша, спавн случится при первом tools/call
        // (ensureLazyServerFor). Экономит ~сотни MB private на idle.
        if (
          LAZY_STDIO_MCP
          && isStdio(state.config)
          && state.status !== 'connected'
          && this.lazySchemaCache[mcpConnectionFingerprint(state.config)]
        ) {
          continue
        }
        promises.push(this.connectServer(id).catch((err) => {
          console.error(`[MCP] Failed to connect "${state.config.name}":`, err instanceof Error ? err.message : err)
        }))
      }
    }
    await Promise.allSettled(promises)
  }

  // ─── Auto-discovery ─────────────────────────────────

  // Discover MCP servers from three sources:
  // 1. Installed plugins: ~/.claude/plugins/marketplaces/<m>/{external_plugins,plugins/mcp}/<p>/.mcp.json
  //    (only if plugin is in installed_plugins.json with valid cache)
  // 2. User's Claude config: ~/.claude.json → mcpServers section
  // 3. Project-level `<cwd>/.mcp.json` for every open tab's cwd
  //
  // All three sources are scanned concurrently with fs.promises to avoid
  // blocking the main process when the user has many plugins or projects.
  private async discoverLocalMcpConfigs(): Promise<void> {
    const claudeDir = path.join(os.homedir(), '.claude')

    // Existing manually-added server names (to avoid duplicates)
    const existingNames = new Set<string>()
    for (const state of this.servers.values()) {
      if (!state.config.autoDiscovered) {
        existingNames.add(state.config.name)
      }
    }

    // Remember the id each auto-discovered server currently has, keyed by name,
    // so the fresh scan can REUSE it instead of minting a new randomUUID every
    // time. A stable id keeps the renderer's selection (and an open Edit form)
    // bound to the same server across the frequent re-scans triggered by tab
    // open/close — otherwise the id churn unmounts the form mid-edit.
    const prevIdsByName = new Map<string, string>()
    for (const [id, state] of this.servers) {
      if (state.config.autoDiscovered) prevIdsByName.set(state.config.name, id)
    }

    // Snapshot each auto-discovered server's SPENT reconnect budget + connection
    // fingerprint BEFORE the delete loop (disconnectServer resets the counter).
    // The re-add below carries it forward for an UNCHANGED config, so a
    // permanently-broken server (module-not-found, bad config) actually reaches
    // the give-up cap instead of being handed a clean budget on every re-scan —
    // which is how a dead server logged a failed connect + fired a churn of
    // `mcp-servers-changed` every ~60s for the life of the app.
    const prevBudget = new Map<string, { attempts: number; fp: string }>()
    for (const [, state] of this.servers) {
      if (state.config.autoDiscovered) {
        prevBudget.set(state.config.name, {
          attempts: state.reconnectAttempts ?? 0,
          fp: mcpConnectionFingerprint(state.config),
        })
      }
    }

    // Remove previously auto-discovered servers (they'll be re-added from fresh scan)
    for (const [id, state] of this.servers) {
      if (state.config.autoDiscovered) {
        this.disconnectServer(id)
        this.servers.delete(id)
      }
    }

    // Helper to add a config if not duplicate
    const addConfig = (config: ExternalMcpServerConfig) => {
      if (existingNames.has(config.name)) return
      if (this.hasServerByName(config.name)) return
      const stableId = prevIdsByName.get(config.name) ?? config.id
      config = stableId === config.id ? config : { ...config, id: stableId }
      this.servers.set(stableId, {
        config,
        status: 'disconnected',
        tools: [],
        toolSchemas: new Map(),
        resources: [],
        resourceTemplates: [],
        prompts: [],
        capabilities: {},
        pendingRequests: new Map(),
        nextRequestId: 1,
        inputBuffer: '',
        testLog: [],
      })
      // Carry the spent reconnect budget across the re-scan IF the connection
      // config is byte-identical to what we last held. A server the user has
      // since fixed has a different fingerprint → clean slate. One that already
      // gave up stays in 'error' so the post-discovery connect loop (which only
      // connects 'disconnected' servers) skips it — no more per-scan churn.
      const prev = prevBudget.get(config.name)
      if (prev && prev.fp === mcpConnectionFingerprint(config)) {
        const st = this.servers.get(stableId)!
        st.reconnectAttempts = prev.attempts
        if (prev.attempts >= MAX_RECONNECT_ATTEMPTS) {
          st.status = 'error'
          st.error = `Auto-reconnect gave up after ${MAX_RECONNECT_ATTEMPTS} attempts (config unchanged since). Toggle the server off/on or fix its config to retry.`
        }
      }
    }

    const pluginsDir = path.join(claudeDir, 'plugins', 'marketplaces')

    // Run the three discovery sources concurrently; each returns a list of
    // ready-to-add configs. We apply them to the internal map sequentially
    // at the end so dedup order stays deterministic.
    const [pluginConfigs, claudeJsonConfigs, projectConfigs] = await Promise.all([
      discoverPluginConfigs(pluginsDir),
      discoverFromClaudeJsonAsync(claudeDir),
      discoverProjectConfigs(this.projectRoots),
    ])

    // Stale manual duplicates: if a name now appears in the discovered
    // claude.json configs but also exists in memory as a non-autoDiscovered
    // entry (leftover from a previous version that stored manual servers
    // separately), drop the stale copy so the fresh one can take over.
    const discoveredNames = new Set([
      ...pluginConfigs.map(c => c.name),
      ...claudeJsonConfigs.map(c => c.name),
      ...projectConfigs.map(c => c.name),
    ])
    for (const [id, state] of this.servers) {
      if (!state.config.autoDiscovered && discoveredNames.has(state.config.name)) {
        this.disconnectServer(id)
        this.servers.delete(id)
        existingNames.delete(state.config.name)
      }
    }

    for (const c of pluginConfigs) addConfig(c)
    for (const c of claudeJsonConfigs) addConfig(c)
    for (const c of projectConfigs) addConfig(c)

    this.notifyChanged()
  }

  /** Check if a server with the given name already exists */
  private hasServerByName(name: string): boolean {
    for (const state of this.servers.values()) {
      if (state.config.name === name) return true
    }
    return false
  }

  // ─── Tool schema export ───────────────────────────────

  /**
   * Get full tool schemas from all connected external servers.
   * Used to register external tools with the server so Claude CLI can see them.
   */
  getExternalToolSchemas(): ExternalToolSchema[] {
    const schemas: ExternalToolSchema[] = []
    const seen = new Set<string>()
    for (const state of this.servers.values()) {
      if (state.status !== 'connected') continue
      for (const [_name, schema] of state.toolSchemas) {
        // Два живых сервера с одноимённым тулом: callTool всё равно выберет
        // первый по порядку итерации — вторая схема была бы ложной рекламой.
        const exposedName = exposedToolName(state, schema.name)
        if (seen.has(exposedName)) continue
        seen.add(exposedName)
        schemas.push({
          name: exposedName,
          description: schema.description || `Tool from ${state.config.name}`,
          inputSchema: schema.inputSchema || { type: 'object', properties: {} },
          serverName: state.config.name,
        })
      }
    }
    // Ленивые stdio: тулы ИЗ ДИСК-КЭША, сервер поднимется при первом вызове.
    for (const state of this.servers.values()) {
      const cached = this.lazyCachedSchemasFor(state)
      if (!cached) continue
      for (const t of cached) {
        const exposedName = exposedToolName(state, t.name)
        if (seen.has(exposedName)) continue
        seen.add(exposedName)
        schemas.push({ ...t, name: exposedName, serverName: state.config.name })
      }
    }
    return schemas
  }

  /** Flatten external MCP resources/prompts into the bridge's single MCP
   * server namespace while retaining enough routing metadata for read/get. */
  getExternalContentCatalog(): ExternalContentCatalog {
    const resources: ExternalContentCatalog['resources'] = []
    const resourceTemplates: ExternalContentCatalog['resourceTemplates'] = []
    const prompts: ExternalContentCatalog['prompts'] = []
    for (const [serverId, state] of this.servers) {
      if (state.status !== 'connected') continue
      const scope = state.config.sourceType === 'plugin' && state.config.pluginName
        ? `plugin_${toolScopePart(state.config.pluginName)}_${toolScopePart(state.config.pluginServerName ?? state.config.name)}`
        : toolScopePart(state.config.name)
      for (const resource of state.resources) {
        resources.push({
          uri: `bridge-mcp://resource/${encodeURIComponent(serverId)}/${Buffer.from(resource.uri).toString('base64url')}`,
          name: `${scope}: ${resource.name}`,
          description: resource.description,
          mimeType: resource.mimeType,
          serverId,
          rawUri: resource.uri,
        })
      }
      for (const template of state.resourceTemplates) {
        resourceTemplates.push({
          // Keep the upstream URI template intact so the MCP client can expand
          // RFC 6570 expressions. Reads carry serverId back through the bridge,
          // avoiding any need to reverse or re-encode the expansion.
          uriTemplate: template.uriTemplate,
          name: `${scope}: ${template.name}`,
          description: template.description,
          mimeType: template.mimeType,
          serverId,
          rawUriTemplate: template.uriTemplate,
        })
      }
      for (const prompt of state.prompts) {
        prompts.push({
          name: `${scope}__${prompt.name}`,
          description: prompt.description,
          arguments: prompt.arguments,
          serverId,
          rawName: prompt.name,
        })
      }
    }
    return { resources, resourceTemplates, prompts }
  }

  async readCatalogResource(uri: string, templateServerId?: string): Promise<any> {
    if (templateServerId) return this.readResource(templateServerId, uri)
    const item = this.getExternalContentCatalog().resources.find(resource => resource.uri === uri)
    if (!item) throw new Error(`External MCP resource not found: ${uri}`)
    return this.readResource(item.serverId, item.rawUri)
  }

  async getCatalogPrompt(name: string, args?: Record<string, unknown>): Promise<any> {
    const item = this.getExternalContentCatalog().prompts.find(prompt => prompt.name === name)
    if (!item) throw new Error(`External MCP prompt not found: ${name}`)
    return this.getPromptContent(item.serverId, item.rawName, args)
  }

  /** Кэш схем для сервера, который сейчас обслуживается ЛЕНИВО (enabled
   *  stdio, не connected/connecting, кэш по fingerprint есть). */
  private lazyCachedSchemasFor(state: McpServerState): CachedToolSchema[] | null {
    if (!LAZY_STDIO_MCP) return null
    if (!state.config.enabled || !isStdio(state.config)) return null
    if (state.status === 'connected' || state.status === 'connecting') return null
    return this.lazySchemaCache[mcpConnectionFingerprint(state.config)] ?? null
  }

  // ─── Public API ───────────────────────────────────────

  /** Get all servers with their status */
  getServers(): ExternalMcpServerInfo[] {
    return Array.from(this.servers.values()).map(s => ({
      ...s.config,
      status: s.status,
      tools: s.tools,
      toolInfos: s.tools.map(name => ({
        name,
        description: (s.toolSchemas.get(name) as any)?.description,
      })),
      resources: s.resources,
      resourceTemplates: s.resourceTemplates,
      prompts: s.prompts,
      error: s.error,
    }))
  }

  /** Get the resources advertised by a connected server. */
  getResources(id: string): McpResourceInfo[] {
    return this.servers.get(id)?.resources ?? []
  }

  /** Get the resource templates advertised by a connected server. */
  getResourceTemplates(id: string): McpResourceTemplateInfo[] {
    return this.servers.get(id)?.resourceTemplates ?? []
  }

  /** Get the prompts advertised by a connected server. */
  getPrompts(id: string): McpPromptInfo[] {
    return this.servers.get(id)?.prompts ?? []
  }

  /** Read the content of a named MCP resource. Returns the raw result object. */
  async readResource(id: string, uri: string): Promise<any> {
    const state = this.servers.get(id)
    if (!state || state.status !== 'connected') throw new Error('Server not connected')
    if (state.config.type === 'ws') {
      return wsJsonRpc(this.ctx(), id, { jsonrpc: '2.0', id: state.nextRequestId++, method: 'resources/read', params: { uri } })
    }
    if (state.config.type === 'http' || state.config.type === 'sse') {
      const res = await httpJsonRpcAuth(this.ctx(), id, { jsonrpc: '2.0', id: state.nextRequestId++, method: 'resources/read', params: { uri } })
      if (res.error) throw new Error(res.error.message)
      return res.result
    }
    return stdioJsonRpc(this.ctx(), id, 'resources/read', { uri })
  }

  /** Get a specific prompt (e.g. fill template with arguments). */
  async getPromptContent(id: string, name: string, args?: Record<string, unknown>): Promise<any> {
    const state = this.servers.get(id)
    if (!state || state.status !== 'connected') throw new Error('Server not connected')
    const body = { jsonrpc: '2.0', id: state.nextRequestId++, method: 'prompts/get', params: { name, ...(args ? { arguments: args } : {}) } }
    if (state.config.type === 'ws') return wsJsonRpc(this.ctx(), id, body)
    if (state.config.type === 'http' || state.config.type === 'sse') {
      const res = await httpJsonRpcAuth(this.ctx(), id, body)
      if (res.error) throw new Error(res.error.message)
      return res.result
    }
    return stdioJsonRpc(this.ctx(), id, 'prompts/get', body.params)
  }

  /** Add a new external MCP server */
  addServer(config: Omit<ExternalMcpServerConfig, 'id'>): ExternalMcpServerInfo {
    const id = randomUUID()
    // Every server added via the bridge UI is written to ~/.claude.json
    // (see saveConfigs), so it's functionally identical to a claude-config
    // entry. Stamp the flags up front so the UI never shows "Manual" for
    // something we just persisted for Claude Code to pick up. sourcePath
    // makes ConnectorDetail render the clickable "C:\Users\…\.claude.json"
    // link — pencil/rider had one after discovery, newly-added servers
    // didn't until the next restart.
    const claudeJsonPath = path.join(os.homedir(), '.claude.json')
    const fullConfig: ExternalMcpServerConfig = {
      ...config,
      id,
      autoDiscovered: config.autoDiscovered ?? true,
      sourceType: config.sourceType ?? 'claude-config',
      sourcePath: config.sourcePath ?? claudeJsonPath,
      description: config.description ?? `From ${path.basename(claudeJsonPath)}`,
    }

    const state: McpServerState = {
      config: fullConfig,
      status: 'disconnected',
      tools: [],
      toolSchemas: new Map(),
      resources: [],
      resourceTemplates: [],
      prompts: [],
      capabilities: {},
      pendingRequests: new Map(),
      nextRequestId: 1,
      inputBuffer: '',
      testLog: [],
    }

    this.servers.set(id, state)
    this.saveConfigs()

    if (fullConfig.enabled) {
      this.connectServer(id).catch((err) => {
        console.error(`[MCP] Failed to connect "${fullConfig.name}":`, err instanceof Error ? err.message : err)
      })
    }

    this.notifyChanged()
    return { ...fullConfig, status: state.status, tools: state.tools }
  }

  /** Remove a server */
  removeServer(id: string): void {
    const state = this.servers.get(id)
    if (!state) return

    const url = state.config.url
    const name = state.config.name
    const wasInClaudeJson = !state.config.sourceType || state.config.sourceType === 'claude-config'
    this.disconnectServer(id)
    this.servers.delete(id)
    // Диск-кэш ленивых схем: запись удалённого сервера не должна «оживлять»
    // его тулы, если новый сервер получит тот же fingerprint.
    const fp = mcpConnectionFingerprint(state.config)
    if (this.lazySchemaCache[fp]) {
      delete this.lazySchemaCache[fp]
      saveToolSchemaCache(this.lazySchemaCache)
    }
    if (wasInClaudeJson) {
      this.removeFromClaudeJson(name)
    }
    this.saveConfigs()
    // Delete any OAuth tokens we had for this server — no point keeping
    // orphan credentials around.
    if (url) {
      try { deleteTokens(url) } catch { /* noop */ }
    }
    this.notifyChanged()
  }

  /** Update an existing server's editable config (name / type / url / headers /
   *  command / args / env / oauth). Preserves identity + source metadata,
   *  re-persists to ~/.claude.json, and reconnects if enabled so the new
   *  endpoint takes effect immediately. Plugin/project-sourced servers are
   *  read-only (their config lives in files we don't own) → returns null. */
  updateServer(id: string, patch: Partial<Omit<ExternalMcpServerConfig, 'id'>>): ExternalMcpServerInfo | null {
    const state = this.servers.get(id)
    if (!state) return null
    const prev = state.config
    if (prev.sourceType === 'plugin' || prev.sourceType === 'project') return null

    const oldName = prev.name
    // Reject a rename that collides with a DIFFERENT server's name — both would
    // serialize to the same `~/.claude.json` key and one would be silently lost
    // on the next save/restart. The UI validates first, but guard here too.
    const newName = patch.name ?? oldName
    if (newName !== oldName) {
      for (const [otherId, s] of this.servers) {
        if (otherId !== id && s.config.name === newName) return null
      }
    }
    // Merge editable fields; keep the id + how/where this server was sourced.
    const next: ExternalMcpServerConfig = {
      ...prev,
      ...patch,
      id: prev.id,
      sourceType: prev.sourceType,
      sourcePath: prev.sourcePath,
      autoDiscovered: prev.autoDiscovered,
    }

    // Tear the live transport down before swapping the endpoint, then reset the
    // discovered surface so stale tools/status don't linger after an edit.
    this.disconnectServer(id)
    state.config = next
    state.tools = []
    state.toolSchemas = new Map()
    state.status = 'disconnected'
    state.error = undefined

    // ~/.claude.json is keyed by NAME and saveConfigs only upserts — a rename
    // would leave the old key orphaned. Drop it first for config-sourced entries.
    const wasInClaudeJson = !prev.sourceType || prev.sourceType === 'claude-config'
    if (wasInClaudeJson && next.name !== oldName) this.removeFromClaudeJson(oldName)
    this.saveConfigs()

    if (next.enabled) {
      this.connectServer(id).catch((err) => {
        console.error(`[MCP] reconnect after edit failed for "${next.name}":`, err instanceof Error ? err.message : err)
      })
    }
    this.notifyChanged()
    return { ...next, status: state.status, tools: state.tools }
  }

  /** Toggle server enabled/disabled */
  async toggleServer(id: string, enabled: boolean): Promise<void> {
    const state = this.servers.get(id)
    if (!state) return

    state.config.enabled = enabled
    if (!state.config.autoDiscovered) this.saveConfigs()

    if (enabled) {
      await this.connectServer(id)
    } else {
      this.disconnectServer(id)
    }
    this.notifyChanged()
    this.onToolsChanged?.()
  }

  /** Test connection to a server. Clears the previous log so the user gets
   *  a clean trace of THIS attempt (otherwise repeated Tests accumulate and
   *  it's unclear which lines are fresh). */
  async testServer(id: string): Promise<{ ok: boolean; tools: string[]; error?: string }> {
    this.clearTestLog(id)
    this.appendLog(id, 'info', '[test] starting connection test')
    try {
      await this.connectServer(id)
      const state = this.servers.get(id)
      const ok = state?.status === 'connected'
      if (ok) {
        this.appendLog(id, 'info', `[test] ok — ${state?.tools.length ?? 0} tools discovered`)
      } else {
        this.appendLog(id, 'error', `[test] failed — ${state?.error || 'unknown error'}`)
      }
      return { ok, tools: state?.tools ?? [], error: state?.error }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      this.appendLog(id, 'error', `[test] exception — ${msg}`)
      return { ok: false, tools: [], error: msg }
    }
  }

  /** Check if a tool name belongs to an external server */
  hasExternalTool(toolName: string): boolean {
    for (const state of this.servers.values()) {
      if (state.status === 'connected' && state.tools.some(raw => exposedToolName(state, raw) === toolName)) {
        return true
      }
    }
    // Ленивый сервер: тул значится в диск-кэше схем (его отдал tools/list и
    // нашёл ToolSearch), а процесс ещё не поднят. Без этой ветки executor
    // отвечал «Unknown tool "browser_navigate"» ДО lazy-подъёма в callTool —
    // тулы «оживали» только после ручного Test сервера (прод-репорт).
    if (LAZY_STDIO_MCP) {
      for (const state of this.servers.values()) {
        if (!state.config.enabled || !isStdio(state.config)) continue
        const cached = this.lazySchemaCache[mcpConnectionFingerprint(state.config)]
        if (cached?.some((t) => exposedToolName(state, t.name) === toolName)) return true
      }
    }
    return false
  }

  /** Execute a tool on an external server */
  async callTool(toolName: string, input: Record<string, unknown>): Promise<McpResult> {
    // Ленивый сервер: тул значится в диск-кэше, процесс ещё не поднят —
    // поднимаем СЕЙЧАС (первое обращение) и продолжаем обычным путём.
    let hasLive = false
    for (const state of this.servers.values()) {
      if (state.status === 'connected' && state.tools.some(raw => exposedToolName(state, raw) === toolName)) { hasLive = true; break }
    }
    if (!hasLive && LAZY_STDIO_MCP) {
      for (const [id, state] of this.servers) {
        // НЕ через lazyCachedSchemasFor: тот исключает 'connecting', а здесь
        // параллельный второй вызов того же тула должен ДОЖДАТЬСЯ уже идущего
        // connectServer (in-flight promise дедуплицирован), а не получить
        // «No external server has tool».
        if (!state.config.enabled || !isStdio(state.config) || state.status === 'connected') continue
        const cached = this.lazySchemaCache[mcpConnectionFingerprint(state.config)]
        if (cached?.some((t) => exposedToolName(state, t.name) === toolName)) {
          console.log(`[MCP] lazy start "${state.config.name}" for tool "${toolName}"`)
          await this.connectServer(id).catch((err) => {
            console.error(`[MCP] lazy connect "${state.config.name}" failed:`, err instanceof Error ? err.message : err)
          })
          break
        }
      }
    }
    for (const [id, state] of this.servers) {
      const rawToolName = state.status === 'connected'
        ? state.tools.find(raw => exposedToolName(state, raw) === toolName)
        : undefined
      if (rawToolName) {
        if (state.config.type === 'sse') {
          const result = await sseJsonRpcRequest(this.ctx(), id, {
            jsonrpc: '2.0',
            id: state.nextRequestId++,
            method: 'tools/call',
            params: { name: rawToolName, arguments: input },
          }) as any
          if (result?.content && Array.isArray(result.content)) return result
          return { content: [{ type: 'text', text: typeof result === 'string' ? result : JSON.stringify(result) }] }
        } else if (state.config.type === 'http') {
          return callHttpTool(this.ctx(), id, rawToolName, input)
        } else if (state.config.type === 'ws') {
          return callWsTool(this.ctx(), id, rawToolName, input)
        } else {
          return callStdioTool(this.ctx(), id, rawToolName, input)
        }
      }
    }
    return { content: [{ type: 'text', text: `Error: No external server has tool "${toolName}"` }] }
  }

  /** Disconnect all servers */
  disconnectAll(): void {
    for (const id of this.servers.keys()) {
      this.disconnectServer(id)
    }
  }

  // ─── Private ────────────────────────────────────────

  /** Дедуп параллельных подключений: второй await того же сервера получает
   *  УЖЕ ИДУЩИЙ промис (иначе гонка ленивых callTool: первый ставит
   *  'connecting', второй не находит сервер и теряет тул-колл). */
  private connectServer(id: string): Promise<void> {
    const inFlight = this.connectInFlight.get(id)
    if (inFlight) return inFlight
    const p = this.doConnectServer(id).finally(() => { this.connectInFlight.delete(id) })
    this.connectInFlight.set(id, p)
    return p
  }

  private async doConnectServer(id: string): Promise<void> {
    const state = this.servers.get(id)
    if (!state) return

    state.status = 'connecting'
    state.error = undefined
    // Явный connect = юзер мог починить путь/конфиг — даём один честный шанс.
    state.permanentFailure = undefined
    this.notifyChanged()

    this.appendLog(id, 'info', `[connect] transport=${state.config.type} name="${state.config.name}"`)
    if (state.config.type === 'stdio' || !state.config.type) {
      this.appendLog(id, 'info', `[connect] command: ${state.config.command} ${(state.config.args ?? []).join(' ')}`)
      if (state.config.env && Object.keys(state.config.env).length > 0) {
        this.appendLog(id, 'info', `[connect] env keys: ${Object.keys(state.config.env).join(', ')}`)
      }
      if (state.config.workingDirectory) {
        this.appendLog(id, 'info', `[connect] cwd: ${state.config.workingDirectory}`)
      }
    } else if (state.config.url) {
      this.appendLog(id, 'info', `[connect] url: ${state.config.url}`)
    }

    try {
      const c = this.ctx()
      if (state.config.type === 'sse') {
        await connectSseLegacy(c, id)
      } else if (state.config.type === 'http') {
        await connectHttp(c, id)
      } else if (state.config.type === 'ws') {
        await connectWs(c, id)
      } else {
        await connectStdio(c, id)
      }
      state.status = 'connected'
      // NOT a counter reset. Connecting only proves the transport came up, not
      // that the server works — and resetting here made the backoff unreachable:
      // a server that connects then dies (unity-mcp: connects, its named pipe
      // isn't there, stderr floods, the circuit breaker kills it) came back to
      // this line every cycle, so `attempts` was forever 0, the delay forever
      // 2s, and MAX_ATTEMPTS never arrived. That respawn loop ran for the life
      // of the app and burned ~1.5 cores of the host spawning processes.
      // The budget is refilled in scheduleReconnect, and only if the connection
      // actually LASTED — see STABLE_CONNECTION_MS.
      state.connectedAt = Date.now()
      try {
        const { emitBridgeHookEvent } = await import('../hooks/emit-bridge-event')
        emitBridgeHookEvent('ConnectorStatusChanged', { name: state.config.name, status: 'connected' })
      } catch { /* ignore */ }
      // Fetch auxiliary capabilities (resources, prompts) if the server advertised
      // them during initialize. Failures here are non-fatal — tools still work.
      await this.fetchExtraCapabilities(id).catch(err => {
        const m = err instanceof Error ? err.message : String(err)
        console.warn(`[MCP] Failed to fetch resources/prompts for "${state.config.name}":`, m)
        this.appendLog(id, 'warn', `[connect] resources/prompts fetch failed: ${m}`)
      })
      this.appendLog(id, 'info', `[connect] done — ${state.tools.length} tools, ${state.resources.length} resources, ${state.resourceTemplates.length} resource templates, ${state.prompts.length} prompts`)
      console.log(`[MCP] Connected to "${state.config.name}" — ${state.tools.length} tools, ${state.resources.length} resources, ${state.resourceTemplates.length} resource templates, ${state.prompts.length} prompts`)
    } catch (err) {
      state.status = 'error'
      try {
        const { emitBridgeHookEvent } = await import('../hooks/emit-bridge-event')
        emitBridgeHookEvent('ConnectorStatusChanged', { name: state.config.name, status: 'error', error: err instanceof Error ? err.message : String(err) })
      } catch { /* ignore */ }
      const baseMsg = err instanceof Error ? err.message : String(err)
      const cause: any = (err as any)?.cause
      const hint: string | null = (err as any)?.hint ?? null
      // Collect all diagnostic fields transports attach (hostname, syscall,
      // TLS code, etc.) so the Test log shows actionable detail instead of
      // Node's bare "fetch failed".
      const extras: string[] = []
      if (cause?.code) extras.push(`code=${cause.code}`)
      if (cause?.errno && cause.errno !== cause?.code) extras.push(`errno=${cause.errno}`)
      if (cause?.hostname) extras.push(`host=${cause.hostname}`)
      if (cause?.syscall) extras.push(`syscall=${cause.syscall}`)
      if (cause?.address) extras.push(`addr=${cause.address}`)
      if (cause?.port) extras.push(`port=${cause.port}`)
      if (cause instanceof Error && cause.message && !baseMsg.includes(cause.message)) {
        extras.push(`cause="${cause.message}"`)
      }
      const fullMsg = extras.length ? `${baseMsg} [${extras.join(' · ')}]` : baseMsg
      state.error = fullMsg
      this.appendLog(id, 'error', `[connect] failed — ${fullMsg}`)
      if (hint) this.appendLog(id, 'warn', `[connect] hint: ${hint}`)
      // `cause` is optional; passing it unconditionally printed a literal
      // "undefined" at the end of every causeless failure ("Circuit breaker:
      // too many stderr errors undefined").
      if (cause === undefined) console.error(`[MCP] Failed to connect to "${state.config.name}":`, fullMsg)
      else console.error(`[MCP] Failed to connect to "${state.config.name}":`, fullMsg, cause)
    }
    this.notifyChanged()
    this.onToolsChanged?.()
  }

  private disconnectServer(id: string): void {
    const state = this.servers.get(id)
    if (state) {
      // Tell the transport-side exit handler that this teardown is on
      // purpose — no auto-reconnect, no log noise about "process exited".
      state.expectedDisconnect = true
      if (state.reconnectTimer) {
        clearTimeout(state.reconnectTimer)
        state.reconnectTimer = undefined
      }
      state.reconnectAttempts = 0
    }
    teardownServer(state)
  }

  /** Issue paginated resource/template/prompt list calls when advertised. */
  private async fetchExtraCapabilities(id: string): Promise<void> {
    const state = this.servers.get(id)!
    const caps = state.capabilities

    if (caps && typeof caps === 'object' && 'resources' in caps) {
      try {
        const resources = await collectMcpList<any>(
          (method, params) => this.rpcRequest(id, method, params),
          'resources/list',
          'resources',
        )
        state.resources = resources.map((resource: any) => ({
          uri: String(resource.uri ?? ''),
          name: String(resource.name ?? resource.uri ?? ''),
          description: resource.description,
          mimeType: resource.mimeType,
        })).filter(resource => resource.uri)
      } catch { /* optional */ }
      try {
        const templates = await collectMcpList<any>(
          (method, params) => this.rpcRequest(id, method, params),
          'resources/templates/list',
          'resourceTemplates',
        )
        state.resourceTemplates = templates.map((template: any) => ({
          uriTemplate: String(template.uriTemplate ?? ''),
          name: String(template.name ?? template.uriTemplate ?? ''),
          description: template.description,
          mimeType: template.mimeType,
        })).filter(template => template.uriTemplate)
      } catch { /* optional; older servers may not implement templates/list */ }
    }
    if (caps && typeof caps === 'object' && 'prompts' in caps) {
      try {
        const prompts = await collectMcpList<any>(
          (method, params) => this.rpcRequest(id, method, params),
          'prompts/list',
          'prompts',
        )
        state.prompts = prompts.map((prompt: any) => ({
          name: String(prompt.name ?? ''),
          description: prompt.description,
          arguments: Array.isArray(prompt.arguments) ? prompt.arguments : undefined,
        })).filter(prompt => prompt.name)
      } catch { /* optional */ }
    }
  }

  /** Transport-agnostic request dispatcher (delegated to connection-lifecycle). */
  private async rpcRequest(id: string, method: string, params: Record<string, unknown>): Promise<unknown> {
    return dispatchRpcRequest(this.ctx(), id, method, params)
  }

  // ─── OAuth & HTTP helpers ───────────────────────────

  /**
   * Build the outgoing headers for an HTTP/SSE MCP request. Resolves ${ENV_VAR}
   * substitution in static headers and, when OAuth is configured, loads stored
   * tokens (refreshing proactively if expired) and sets Authorization.
   */
  private async buildHttpHeaders(id: string): Promise<Record<string, string>> {
    const state = this.servers.get(id)!
    const out: Record<string, string> = {}
    if (state.config.headers) {
      for (const [k, v] of Object.entries(state.config.headers)) {
        out[k] = resolveEnvVars(v)
      }
    }
    if (state.config.oauth && state.config.url) {
      const tokens = loadTokens(state.config.url)
      if (tokens) {
        // Proactively refresh if expiring within 60 seconds.
        if (tokens.expires_at && tokens.expires_at < Date.now() + 60_000 && tokens.refresh_token) {
          await this.tryRefreshTokens(id).catch(() => {})
        }
        const fresh = (state.config.url ? loadTokens(state.config.url) : null) ?? tokens
        out['Authorization'] = `Bearer ${fresh.access_token}`
      }
    }
    return out
  }

  // ─── OAuth thin delegations — implementation lives in `oauth-bridge.ts`
  // ───────────────────────────────────────────────────────────────────────

  /** Persist a discovered OAuth Authorization Server URL. Called from the HTTP
   *  transport when it sees a 401 with an RFC 9728 challenge. */
  private setDiscoveredOAuthMetadata(id: string, asUrl: string, scopes?: string[]): void {
    if (oauthBridge.setDiscoveredOAuthMetadata(this.servers.get(id), asUrl, scopes)) {
      this.saveConfigs()
      this.notifyChanged()
    }
  }

  /** In-flight refresh — coalesces concurrent callers per server id. */
  private async tryRefreshTokens(id: string): Promise<boolean> {
    return oauthBridge.tryRefreshTokens(id, this.servers.get(id))
  }

  /** Save freshly acquired tokens (called from the OAuth flow launcher). */
  saveOAuthTokens(id: string, tokens: StoredTokens): void {
    oauthBridge.saveOAuthTokensFor(this.servers.get(id), tokens)
  }

  /** Merge newly registered OAuth credentials (Dynamic Client Registration). */
  setOAuthClientCredentials(id: string, clientId: string, clientSecret?: string): void {
    if (oauthBridge.setOAuthClientCredentialsFor(this.servers.get(id), clientId, clientSecret)) {
      this.saveConfigs()
      this.notifyChanged()
    }
  }

  /** Remove stored tokens for a server (logout). */
  clearOAuthTokens(id: string): void {
    oauthBridge.clearOAuthTokensFor(this.servers.get(id))
  }

  /** Check whether a server currently has valid-looking stored tokens. */
  hasOAuthTokens(id: string): boolean {
    return oauthBridge.hasOAuthTokensFor(this.servers.get(id))
  }

  /** Public accessor for the authorization server metadata of a configured server. */
  async getOAuthMetadata(id: string): Promise<OAuthServerMetadata | null> {
    return oauthBridge.getOAuthMetadataFor(this.servers.get(id))
  }

  // ─── Server → client message handling ────────────────
  //
  // Messages from the remote MCP server may be:
  //  - a response to a request we sent (has `id` matching a pending entry)
  //  - a request from the server (has `id` + `method`) — e.g. elicitation or sampling
  //  - a notification (has `method`, no `id`) — progress, cancellation, log, etc.

  /** Build the slim ctx the message handlers need. Rebuilt per-call rather
   *  than cached because `onToolsChanged` is mutable on the manager. */
  private msgCtx(): MessageHandlerCtx {
    return {
      servers: this.servers,
      window: this.window,
      notifyChanged: () => this.notifyChanged(),
      onToolsChanged: this.onToolsChanged,
      rpcRequest: (id, method, params) => this.rpcRequest(id, method, params),
    }
  }

  /** Top-level inbound dispatcher (request / notification / response).
   *  Implementation lives in `server-message-handlers.ts`. */
  private handleServerMessage(
    serverId: string,
    msg: any,
    sendResponse: (response: Record<string, unknown>) => void,
  ): void {
    handleServerMessageImpl(this.msgCtx(), serverId, msg, sendResponse)
  }

  // ─── Persistence ────────────────────────────────────

  /** Serializes concurrent `.claude.json` writes — losing a write here would
   *  drop the user's MCP servers on the next startup. */
  private saveChain: Promise<void> = Promise.resolve()

  /** Upsert our current view of manually-added + claude-config servers into
   *  `~/.claude.json`. Never deletes entries that are missing from memory —
   *  during a re-discovery window `this.servers` is briefly empty of auto-
   *  discovered entries, and a save triggered in that window would wipe the
   *  user's config. Explicit removal goes through `removeFromClaudeJson`. */
  private saveConfigs(): void {
    const ours = Array.from(this.servers.values())
      .map(s => s.config)
      .filter(c => c.sourceType !== 'plugin' && c.sourceType !== 'project')

    // Clear the legacy electron-store cache — we migrated to ~/.claude.json.
    try { getStore().set('mcpServers', []) } catch { /* noop */ }

    this.saveChain = this.saveChain
      .then(() => mutateClaudeJson((mcpServers) => {
        for (const c of ours) {
          mcpServers[c.name] = configToClaudeJsonEntry(c)
        }
      }))
      .catch(err => console.error('[MCP] Failed to persist to ~/.claude.json:', err))
  }

  /** Explicitly remove a server's entry from `~/.claude.json`. Called from
   *  `removeServer` when the user deletes a claude-config-sourced entry. */
  private removeFromClaudeJson(name: string): void {
    this.saveChain = this.saveChain
      .then(() => mutateClaudeJson((mcpServers) => {
        delete mcpServers[name]
      }))
      .catch(err => console.error('[MCP] Failed to remove from ~/.claude.json:', err))
  }

  /** Снять схемы живых stdio-серверов в диск-кэш (для ленивого старта). */
  private harvestSchemaCache(): void {
    let dirty = false
    for (const state of this.servers.values()) {
      if (state.status !== 'connected' || !isStdio(state.config) || state.toolSchemas.size === 0) continue
      const fp = mcpConnectionFingerprint(state.config)
      const schemas: CachedToolSchema[] = [...state.toolSchemas.values()].map((t) => ({
        name: t.name,
        description: t.description || `Tool from ${state.config.name}`,
        inputSchema: t.inputSchema || { type: 'object', properties: {} },
      }))
      if (JSON.stringify(this.lazySchemaCache[fp]) !== JSON.stringify(schemas)) {
        this.lazySchemaCache[fp] = schemas
        dirty = true
      }
    }
    if (dirty) saveToolSchemaCache(this.lazySchemaCache)
  }

  private notifyChanged(): void {
    if (this.window.isDestroyed()) return
    // A trailing fire is already scheduled — it will pick up the latest state.
    if (this.notifyTimer) return
    const wait = Math.max(0, McpServerManager.NOTIFY_MIN_INTERVAL_MS - (Date.now() - this.notifyLastAt))
    this.notifyTimer = setTimeout(() => {
      this.notifyTimer = null
      this.notifyLastAt = Date.now()
      // Сбор диск-кэша схем — ВНУТРИ throttle-окна: notifyChanged дергается
      // с ~20 мест, включая reconnect-штормы, а harvest делает JSON.stringify
      // по всем stdio-серверам и синхронную запись файла при изменении.
      this.harvestSchemaCache()
      if (this.window.isDestroyed()) return
      this.window.webContents.send('mcp-servers-changed', this.getServers())
    }, wait)
  }

  // ─── Test log ───────────────────────────────────────

  /** Append a line to the per-server ring buffer AND broadcast it to the
   *  renderer. The UI's TestLogPane subscribes to `mcp:test-log` for live
   *  updates; if it mounts mid-connect, `getTestLog` replays the buffer. */
  private appendLog(id: string, level: McpTestLogEntry['level'], msg: string): void {
    appendLogShared(this.window, this.servers.get(id), id, level, msg)
  }

  /** Read the full ring buffer for a server. Used by the UI on mount to
   *  populate the log pane before any live events arrive. */
  getTestLog(id: string): McpTestLogEntry[] {
    return getTestLogShared(this.servers.get(id))
  }

  /** Wipe the ring buffer for a server — called when the user hits "Clear"
   *  in the log pane, or implicitly at the start of each new connect so old
   *  noise doesn't mix with fresh output. */
  clearTestLog(id: string): void {
    clearTestLogShared(this.window, this.servers.get(id), id)
  }
}
