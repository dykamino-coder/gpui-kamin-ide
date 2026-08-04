import { signal, computed } from '@preact/signals'
import type { HealthCheck, ExtendedStats } from '../types'

export interface PtySessionInfo {
  id: string
  userName: string
  cwd: string
  state: string
  createdAt: string
  lastActivityAt: string
  mcpCallCount: number
  inputCount: number
  mcpInitialized?: boolean
  mcpLastError?: string | null
  sessionTitle?: string | null
  cliConversationId?: string | null
  // Live aggregates derived from jsonl_events (filled when broadcast).
  userMessages?: number
  assistantMessages?: number
  compactCount?: number
  totalTokens?: number
  contextTokens?: number
  model?: string | null
  // Live resource use over the session's process tree (Linux only; merged from
  // the sessions REST endpoint on a 5s poll). null when unknown / non-Linux.
  cpuPercent?: number | null
  memBytes?: number | null
  // Client presence (from the sessions REST merge): false = no client bound,
  // waiting out the detach-grace window before the reaper kills it.
  attached?: boolean
  detachedForSec?: number | null
}

export const serverHealth = signal<HealthCheck | null>(null)
export const healthCheckedAt = signal<Date | null>(null)
export const serverStats = signal<ExtendedStats | null>(null)
export const isConnected = signal(false)
export const terminalSessions = signal(0)
export const ptySessions = signal<PtySessionInfo[]>([])
export const serverStatus = signal<{ os?: string; nodeVersion?: string; startedAt?: string } | null>(null)
export const cachedAccount = signal<{ email?: string; organization?: string; subscriptionType?: string } | null>(null)
export const accountCheckedAt = signal<Date | null>(null)
export const cachedUsage = signal<unknown>(null)

export const uptime = computed(() => {
  const stats = serverStats.value
  if (!stats?.startedAt) return '---'
  const diff = Date.now() - new Date(stats.startedAt).getTime()
  const h = Math.floor(diff / 3600000), m = Math.floor((diff % 3600000) / 60000), s = Math.floor((diff % 60000) / 1000)
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m ${s}s`
  return `${s}s`
})
// Legacy proxy-mode counters removed — Stats panel + Stats grid now
// derive everything from /api/dashboard/stats/overview (jsonl_events).
// `userRequests` kept as a stable per-user color seed (user-colors.ts).
export const userRequests = computed(() => serverStats.value?.userRequests ?? {})
