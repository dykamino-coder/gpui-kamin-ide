import { ipcMain, type IpcMainInvokeEvent } from 'electron'
import type { McpServerManager } from '../mcp/mcp-server-manager'
import type { ExternalMcpServerConfig } from '../../shared/types'

export interface McpIpcContext {
  getMcpServerManager: () => McpServerManager | null
  openUrlInBrowser: (url: string) => Promise<void>
}

export function registerMcpIPC(ctx: McpIpcContext): void {
  const mgr = () => ctx.getMcpServerManager()

  ipcMain.handle('mcp:get-servers', () => {
    return mgr()?.getServers() ?? []
  })

  // MCP subsystem readiness for the webview's input-bar gate. Was UNHANDLED, so
  // every getMcpStatus() logged "unimplemented invoke channel: mcp:status" — and
  // the webview re-polls it on each reconnect/iframe-reload, so a churn storm
  // (e.g. a freeze→reload loop) spammed the host log with hundreds of warns.
  // Ready = the manager is initialized (the webview treats anything but `false`
  // as ready; see useBridgeListeners).
  ipcMain.handle('mcp:status', () => {
    return mgr() != null
  })

  ipcMain.handle('mcp:add-server', (_event: IpcMainInvokeEvent, config: Omit<ExternalMcpServerConfig, 'id'>) => {
    return mgr()?.addServer(config) ?? null
  })

  ipcMain.handle('mcp:update-server', (_event: IpcMainInvokeEvent, id: string, config: Partial<Omit<ExternalMcpServerConfig, 'id'>>) => {
    return mgr()?.updateServer(id, config) ?? null
  })

  ipcMain.handle('mcp:remove-server', (_event: IpcMainInvokeEvent, id: string) => {
    mgr()?.removeServer(id)
  })

  ipcMain.handle('mcp:toggle-server', (_event: IpcMainInvokeEvent, id: string, enabled: boolean) => {
    return mgr()?.toggleServer(id, enabled)
  })

  ipcMain.handle('mcp:test-server', (_event: IpcMainInvokeEvent, id: string) => {
    return mgr()?.testServer(id) ?? { ok: false, tools: [], error: 'Manager not initialized' }
  })

  // Replay the per-server ring buffer.
  ipcMain.handle('mcp:get-test-log', (_event: IpcMainInvokeEvent, id: string) => {
    return mgr()?.getTestLog(id) ?? []
  })

  ipcMain.handle('mcp:clear-test-log', (_event: IpcMainInvokeEvent, id: string) => {
    mgr()?.clearTestLog(id)
  })

  // ─── OAuth flow for MCP servers ──────────────────────
  ipcMain.handle('mcp:oauth-connect', async (_event: IpcMainInvokeEvent, id: string) => {
    const mcpServerManager = mgr()
    if (!mcpServerManager) return { ok: false, error: 'Manager not initialized' }
    const server = mcpServerManager.getServers().find(s => s.id === id)
    if (!server?.oauth) return { ok: false, error: 'Server is not configured for OAuth' }

    try {
      const { discoverAuthorizationServerMetadata, authorize, registerOAuthClient } = await import('../mcp/oauth-flow')
      const { findClaudeCodeEntryByServerUrl } = await import('../mcp/claude-code-import')
      const metadata = await discoverAuthorizationServerMetadata(server.oauth.authServerUrl)
      const callbackPort = server.oauth.callbackPort && server.oauth.callbackPort > 0
        ? server.oauth.callbackPort
        : 38976
      const redirectUri = `http://localhost:${callbackPort}/callback`

      let clientId = server.oauth.clientId
      let clientSecret = server.oauth.clientSecret

      if ((!clientId || clientId.trim() === '') && server.url) {
        const cached = findClaudeCodeEntryByServerUrl(server.url)
        if (cached?.clientId) {
          clientId = cached.clientId
          clientSecret = cached.clientSecret
          mcpServerManager.setOAuthClientCredentials(id, clientId, clientSecret)
          if (cached.accessToken) {
            mcpServerManager.saveOAuthTokens(id, {
              access_token: cached.accessToken,
              refresh_token: cached.refreshToken,
              expires_at: typeof cached.expiresAt === 'number' ? cached.expiresAt : undefined,
              authServerUrl: server.oauth.authServerUrl,
              lastRefreshAt: Date.now(),
            })
            await mcpServerManager.toggleServer(id, true)
            return { ok: true }
          }
        }
      }

      if ((!clientId || clientId.trim() === '') && metadata.registration_endpoint) {
        const candidateNames = ['Claude Code', 'Codex', 'Cursor', `Open Claude Bridge · ${server.name}`]
        let registered: { client_id: string; client_secret?: string } | null = null
        let lastErr: Error | null = null
        for (const clientName of candidateNames) {
          try {
            registered = await registerOAuthClient(metadata.registration_endpoint, {
              client_name: clientName,
              redirect_uris: [redirectUri],
              scope: server.oauth.scope,
            })
            break
          } catch (e) {
            lastErr = e instanceof Error ? e : new Error(String(e))
            if (!lastErr.message.includes('HTTP 403')) throw lastErr
          }
        }
        if (!registered) throw lastErr ?? new Error('DCR failed for all allowlisted client_name values')
        clientId = registered.client_id
        clientSecret = registered.client_secret
        mcpServerManager.setOAuthClientCredentials(id, clientId, clientSecret)
      }
      if (!clientId || clientId.trim() === '') {
        return { ok: false, error: 'No clientId available — DCR failed and no Claude Code credentials were found to import. Run the server in Claude Code once, or paste clientId/clientSecret manually.' }
      }
      const tokens = await authorize({
        metadata,
        clientId,
        clientSecret,
        scope: server.oauth.scope,
        callbackPort,
        serverId: id,
        openAuthorizationUrl: async (url) => { await ctx.openUrlInBrowser(url) },
      })
      mcpServerManager.saveOAuthTokens(id, {
        ...tokens,
        authServerUrl: server.oauth.authServerUrl,
        lastRefreshAt: Date.now(),
      })
      await mcpServerManager.toggleServer(id, true)
      return { ok: true }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })

  ipcMain.handle('mcp:oauth-disconnect', (_event: IpcMainInvokeEvent, id: string) => {
    mgr()?.clearOAuthTokens(id)
    return { ok: true }
  })

  ipcMain.handle('mcp:oauth-status', (_event: IpcMainInvokeEvent, id: string) => {
    return { connected: mgr()?.hasOAuthTokens(id) ?? false }
  })

  // ─── MCP resources / prompts ────────────────────────
  ipcMain.handle('mcp:get-resources', (_event: IpcMainInvokeEvent, id: string) => {
    return mgr()?.getResources(id) ?? []
  })
  ipcMain.handle('mcp:read-resource', async (_event: IpcMainInvokeEvent, id: string, uri: string) => {
    try {
      const result = await mgr()?.readResource(id, uri)
      return { ok: true, result }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })
  ipcMain.handle('mcp:get-prompts', (_event: IpcMainInvokeEvent, id: string) => {
    return mgr()?.getPrompts(id) ?? []
  })
  ipcMain.handle('mcp:get-prompt', async (_event: IpcMainInvokeEvent, id: string, name: string, args?: Record<string, unknown>) => {
    try {
      const result = await mgr()?.getPromptContent(id, name, args)
      return { ok: true, result }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })

  // ─── MCP official registry ───
  ipcMain.handle('mcp:registry-list', async () => {
    try {
      const res = await fetch('https://api.anthropic.com/mcp-registry/v0/servers', {
        headers: { 'User-Agent': 'open-claude-bridge' },
      })
      if (!res.ok) return { ok: false, error: `HTTP ${res.status}`, servers: [] }
      const data = await res.json() as any
      const servers = Array.isArray(data?.servers) ? data.servers : Array.isArray(data) ? data : []
      return { ok: true, servers }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err), servers: [] }
    }
  })
}
