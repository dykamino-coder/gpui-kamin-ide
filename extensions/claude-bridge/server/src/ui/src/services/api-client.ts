import type { RequestLogEntry, ServerConfig, ExtendedStats, UserTimeSeriesEntry } from '../types'
import { getSessionToken, isAuthenticated } from '../signals/auth'
import type { PlanUsageData as UsageData } from '../../../shared/plan-usage'
import type { McpUsageReport } from '../../../shared/mcp-report'
import {
  toCodexError,
  type CodexAccountStatus,
  type CodexDeviceLogin,
  type CodexErrorInfo,
} from '../components/dashboard/codex-account-model'

function authHeaders(): Record<string, string> {
  const token = getSessionToken()
  return token ? { Authorization: `Bearer ${token}` } : {}
}

/** Handle 401 responses — clear auth state */
function handle401(res: Response): void {
  if (res.status === 401) {
    isAuthenticated.value = false
    localStorage.removeItem('dashboard_session_token')
  }
}

async function fetchJson<T>(path: string): Promise<T> {
  const res = await fetch(path, { headers: authHeaders() })
  handle401(res)
  if (!res.ok) throw new Error(`HTTP ${res.status}`)
  return res.json()
}

async function postJson<T>(path: string): Promise<T> {
  const res = await fetch(path, { method: 'POST', headers: authHeaders() })
  handle401(res)
  if (!res.ok) throw new Error(`HTTP ${res.status}`)
  return res.json()
}

/** Thrown by codex account calls; carries the server's plain-language `{ error }`. */
export class CodexApiError extends Error {
  constructor(public readonly info: CodexErrorInfo) {
    super(info.message)
  }
}

async function codexPost<T>(path: string): Promise<T> {
  const res = await fetch(path, { method: 'POST', headers: authHeaders() })
  handle401(res)
  if (!res.ok) {
    const body = await res.json().catch(() => null)
    throw new CodexApiError(toCodexError(body, `HTTP ${res.status}`))
  }
  return res.json()
}

async function deleteJson<T>(path: string): Promise<T> {
  const res = await fetch(path, { method: 'DELETE', headers: authHeaders() })
  handle401(res)
  if (!res.ok) throw new Error(`HTTP ${res.status}`)
  return res.json()
}

export const api = {
  getMcpReport: () => fetchJson<McpUsageReport>('/api/dashboard/mcp-report'),
  getSessions: () =>
    fetchJson<{
      sessions: Array<{
        id: string
        userName: string
        cwd: string
        state: string
        createdAt: string
        lastActivityAt: string
        mcpCallCount: number
        durationSec: number
        idleSec: number
      }>
    }>('/api/dashboard/sessions'),
  killSession: (id: string) =>
    postJson<{ ok: boolean; id: string }>(`/api/dashboard/sessions/${encodeURIComponent(id)}/kill`),
  getStatus: () => fetchJson<Record<string, unknown>>('/api/dashboard/status'),
  getStats: () => fetchJson<ExtendedStats>('/api/dashboard/stats'),
  getRequests: (limit = 50, offset = 0) =>
    fetchJson<RequestLogEntry[]>(`/api/dashboard/requests?limit=${limit}&offset=${offset}`),
  getFilteredRequests: (filter: { endpoint?: string; userName?: string; status?: string }, limit = 50, offset = 0) => {
    const params = new URLSearchParams()
    params.set('limit', String(limit))
    params.set('offset', String(offset))
    if (filter.endpoint) params.set('endpoint', filter.endpoint)
    if (filter.userName) params.set('userName', filter.userName)
    if (filter.status) params.set('status', filter.status)
    return fetchJson<RequestLogEntry[]>(`/api/dashboard/requests?${params.toString()}`)
  },
  getUserChat: (userName: string) =>
    fetchJson<RequestLogEntry[]>(`/api/dashboard/user-chat/${encodeURIComponent(userName)}`),
  getRequest: (id: string) => fetchJson<RequestLogEntry>(`/api/dashboard/requests/${id}`),
  getConfig: () => fetchJson<ServerConfig>('/api/dashboard/config'),
  refreshHealth: (prompt?: string, force = false) => {
    const params = new URLSearchParams()
    if (prompt) params.set('prompt', prompt)
    if (force) params.set('force', 'true')
    const qs = params.toString()
    return postJson<Record<string, unknown>>(`/api/dashboard/health/refresh${qs ? `?${qs}` : ''}`)
  },
  getAccount: (force = false) => fetchJson<AccountInfo>(`/api/dashboard/account${force ? '?force=true' : ''}`),
  getUsage: (force = false) => fetchJson<UsageData>(`/api/dashboard/usage${force ? '?force=1' : ''}`),
  getAuthStatus: (force = false) =>
    fetchJson<{ loggedIn: boolean; authMethod?: string; apiProvider?: string; error?: string }>(
      `/api/dashboard/auth-status${force ? '?force=true' : ''}`,
    ),
  // Shared ChatGPT (codex) account of this Bridge server.
  getCodexLimits: (force = false) =>
    fetchJson<{ limits: CodexRateLimits | null }>(`/api/dashboard/codex/limits${force ? '?force=true' : ''}`),
  getCodexAccount: (force = false) =>
    fetchJson<CodexAccountStatus>(`/api/dashboard/codex/account${force ? '?force=true' : ''}`),
  startCodexLogin: () => codexPost<CodexDeviceLogin>('/api/dashboard/codex/login'),
  cancelCodexLogin: () => codexPost<{ ok: true }>('/api/dashboard/codex/login/cancel'),
  logoutCodex: () => codexPost<{ ok: true }>('/api/dashboard/codex/logout'),
  clearHistory: () => deleteJson<{ ok: boolean }>('/api/dashboard/requests'),
  deleteByFilter: (filter: { endpoint?: string; userName?: string; status?: string }) => {
    const params = new URLSearchParams()
    if (filter.endpoint) params.set('endpoint', filter.endpoint)
    if (filter.userName) params.set('userName', filter.userName)
    if (filter.status) params.set('status', filter.status)
    return deleteJson<{ ok: boolean; deleted: number }>(`/api/dashboard/requests/filter?${params.toString()}`)
  },
  getUserTimeSeries: (period: string = 'day', days: number = 30) =>
    fetchJson<UserTimeSeriesEntry[]>(`/api/dashboard/stats/timeseries?period=${period}&days=${days}`),
  getUsers: (engine: 'all' | 'claude' | 'codex' = 'all') =>
    fetchJson<UserSummary[]>(`/api/dashboard/users${engine === 'all' ? '' : `?engine=${engine}`}`),
  getUserRequests: (userName: string, limit = 50) =>
    fetchJson<RequestLogEntry[]>(`/api/dashboard/users/${encodeURIComponent(userName)}/requests?limit=${limit}`),

  // Proxy settings
  getProxySettings: () => fetchJson<ProxySettings>('/api/dashboard/proxy-settings'),
  updateProxySettings: async (settings: Partial<ProxySettings>) => {
    const res = await fetch('/api/dashboard/proxy-settings', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json', ...authHeaders() },
      body: JSON.stringify(settings),
    })
    handle401(res)
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    return res.json() as Promise<ProxySettings>
  },
  resetProxySettings: () => deleteJson<ProxySettings>('/api/dashboard/proxy-settings'),
  updateToken: async (id: string, patch: { name?: string }) => {
    const res = await fetch(`/api/dashboard/tokens/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json', ...authHeaders() },
      body: JSON.stringify(patch),
    })
    handle401(res)
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    return res.json()
  },
  uploadCaCert: async (file: File) => {
    const formData = new FormData()
    formData.append('cert', file)
    const res = await fetch('/api/dashboard/proxy-settings/cert', {
      method: 'POST',
      headers: authHeaders(),
      body: formData,
    })
    handle401(res)
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    return res.json() as Promise<ProxySettings & { path: string }>
  },
}

export interface ProxySettings {
  httpProxy: string | null
  httpsProxy: string | null
  noProxy: string | null
  caCert: string | null
  logLevel: string
  maxRequests: number
  [key: string]: unknown
}

export interface AccountInfo {
  email: string | null
  plan: string | null
  organization?: string | null
  apiKeySource?: string | null
  expiresAt?: number | null
  /** End of the stored claude.ai sign-in (ISO), from `claude auth status`. */
  loginExpiresAt?: string | null
  displayName?: string | null
  error?: string
}

export interface UserEngineSummary {
  sessions: number
  totalInputs: number
  responses: number
  tokens: { input: number; output: number; cacheRead: number; cacheCreation: number }
}

import type { CodexRateLimits } from '../../../shared/codex-rate-limits'
export type { CodexRateLimits, CodexLimitBucket, CodexLimitWindow } from '../../../shared/codex-rate-limits'

export interface UserSummary {
  userName: string
  activeSessions: number
  totalInputs: number
  tokens: { input: number; output: number; cacheRead: number; cacheCreation: number }
  contextTokens: number
  /** Claude responses at Anthropic API rates. */
  cost: number
  /** ChatGPT (codex): billed by the ChatGPT plan; API-equivalent list price. */
  codex?: { apiEquivalentUsd: number; unpricedTokens: number; unpricedModels: string[]; pricesAsOf: string }
  /** The same totals per engine (absent on older servers). */
  engines?: Partial<Record<'claude' | 'codex', UserEngineSummary>>
  firstSeen: string | null
  lastSeen: string | null
}

export type { PlanUsageData as UsageData } from '../../../shared/plan-usage'
