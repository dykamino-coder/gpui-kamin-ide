import { serverHealth, healthCheckedAt, serverStats, isConnected, terminalSessions, ptySessions, cachedAccount, accountCheckedAt, cachedUsage, type PtySessionInfo } from '../signals/server'
import { addRequest, updateRequest, addError, seedErrors } from '../signals/requests'
import type { BridgeEvent, InitData, ExtendedStats, HealthCheck, RequestLogEntry } from '../types'

class DashboardWSClient {
  private ws: WebSocket | null = null
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private url = ''
  onReconnect: (() => void) | null = null

  connect(url: string) {
    this.url = url
    this.doConnect()
  }

  disconnect() {
    if (this.reconnectTimer) { clearTimeout(this.reconnectTimer); this.reconnectTimer = null }
    if (this.ws) { this.ws.close(); this.ws = null }
    isConnected.value = false
  }

  send(msg: object) {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(msg))
  }

  private doConnect() {
    try {
      this.ws = new WebSocket(this.url)
      this.ws.onopen = () => { isConnected.value = true }
      this.ws.onclose = () => { isConnected.value = false; this.scheduleReconnect() }
      this.ws.onerror = () => { isConnected.value = false }
      this.ws.onmessage = (e: MessageEvent) => {
        try { this.handleMessage(JSON.parse(e.data)) } catch {}
      }
    } catch { this.scheduleReconnect() }
  }

  private scheduleReconnect() {
    if (this.reconnectTimer) return
    this.reconnectTimer = setTimeout(() => { this.reconnectTimer = null; this.doConnect() }, 3000)
  }

  private handleMessage(msg: BridgeEvent) {
    switch (msg.type) {
      case 'init': {
        const d = msg.data as InitData & { terminalSessions?: number; ptySessions?: PtySessionInfo[]; account?: unknown; healthCheckedAt?: number; accountCheckedAt?: number; usage?: unknown }
        serverStats.value = d.stats
        if (d.health) { serverHealth.value = d.health; healthCheckedAt.value = d.healthCheckedAt ? new Date(d.healthCheckedAt) : new Date() }
        if (d.account) { cachedAccount.value = d.account as any }
        if (d.accountCheckedAt) { accountCheckedAt.value = new Date(d.accountCheckedAt) }
        if (d.usage) { cachedUsage.value = d.usage }
        if (d.stats?.requestLog) seedErrors(d.stats.requestLog)
        if (typeof d.terminalSessions === 'number') terminalSessions.value = d.terminalSessions
        if (d.ptySessions) ptySessions.value = d.ptySessions
        this.onReconnect?.()
        break
      }
      case 'stats:updated': serverStats.value = msg.data as ExtendedStats; break
      case 'health:updated': serverHealth.value = msg.data as HealthCheck; healthCheckedAt.value = new Date(); break
      case 'account:updated': cachedAccount.value = msg.data as any; accountCheckedAt.value = new Date(); break
      case 'usage:updated': cachedUsage.value = msg.data; break
      case 'request:start': addRequest(msg.data as RequestLogEntry); break
      case 'request:complete': {
        const e = msg.data as RequestLogEntry
        updateRequest(e.id, e)
        if (e.status === 'error') addError(e)
        break
      }
      case 'terminal:sessions': {
        const t = msg.data as { count: number }
        terminalSessions.value = t.count
        break
      }
      case 'session:created': {
        const s = msg.data as { sessionId: string; userName: string; cwd: string; state: string; createdAt: string; lastActivityAt: string; mcpCallCount: number; inputCount: number }
        ptySessions.value = [...ptySessions.value, {
          id: s.sessionId,
          userName: s.userName,
          cwd: s.cwd || '',
          state: s.state || 'running',
          createdAt: s.createdAt,
          lastActivityAt: s.lastActivityAt,
          mcpCallCount: s.mcpCallCount || 0,
          inputCount: s.inputCount || 0,
        }]
        break
      }
      case 'session:updated': {
        const u = msg.data as { sessionId: string; mcpCallCount?: number; inputCount?: number; mcpInitialized?: boolean; mcpLastError?: string | null; lastActivityAt?: string }
        ptySessions.value = ptySessions.value.map(p => {
          if (p.id !== u.sessionId) return p
          const patch: Partial<PtySessionInfo> = {}
          if (u.mcpCallCount !== undefined) patch.mcpCallCount = u.mcpCallCount
          if (u.inputCount !== undefined) patch.inputCount = u.inputCount
          if (u.mcpInitialized !== undefined) patch.mcpInitialized = u.mcpInitialized
          if (u.mcpLastError !== undefined) patch.mcpLastError = u.mcpLastError
          if (u.lastActivityAt) patch.lastActivityAt = u.lastActivityAt
          return { ...p, ...patch }
        })
        break
      }
      case 'session:destroyed': {
        const s = msg.data as { sessionId: string }
        ptySessions.value = ptySessions.value.filter(p => p.id !== s.sessionId)
        break
      }
    }
  }
}

export const wsClient = new DashboardWSClient()
