import { useEffect, useState } from 'preact/compat'
import { signal } from '@preact/signals'
import { StatusCard } from './components/dashboard/StatusCard'
import { ErrorLog } from './components/errors/ErrorLog'
import { TokenSessionsList, TokensStatsGrid } from './components/dashboard/StatsCard'
import { selectedTokenId, selectTokenId } from './signals/ui'
import { Modal } from './components/shared'
import {
  serverHealth,
  serverStats,
  serverStatus,
  isConnected,
  ptySessions,
  cachedAccount,
  accountCheckedAt,
  cachedUsage,
} from './signals/server'
import { errorLog, clearErrors, capturedLog } from './signals/requests'
import { api } from './services/api-client'
import { wsClient } from './services/ws-client'
import type { UsageData, AccountInfo } from './services/api-client'
import { UsageBars } from './components/dashboard/UsageBars'
import { CodexAccountCard } from './components/dashboard/CodexAccountCard'
import { codexStatusRow, type CodexStatus } from './components/dashboard/codex-account-model'
import { UsageChart } from './components/dashboard/UsageChart'
import { ApiErrorsCard } from './components/dashboard/ApiErrorsCard'
import { McpUsageReport } from './components/dashboard/McpUsageReport'
import { ActivePtySessions } from './components/dashboard/ActivePtySessions'
import { engineBreakdown, ptyUserKey } from './components/dashboard/pty-groups'

import { navigateToConsole } from './router'
import styles from './SingleDashboard.module.css'

const showHealthModal = signal(false)
const showResponseModal = signal(false)
const showErrorsModal = signal(false)
const selectedTokenSessionCount = signal<number | null>(null)

function SelectedTokenModal() {
  const id = selectedTokenId.value
  const count = selectedTokenSessionCount.value
  const title = id ? (count != null ? `${id} — Sessions (${count})` : `${id} — Sessions`) : 'Sessions'
  return (
    <Modal
      isOpen={!!id}
      onClose={() => {
        selectedTokenSessionCount.value = null
        selectTokenId(null)
      }}
      title={title}
      size="lg"
    >
      {id && (
        <TokenSessionsList
          tokenId={id}
          onCountChange={(n) => {
            selectedTokenSessionCount.value = n
          }}
        />
      )}
    </Modal>
  )
}
const healthRefreshing = signal(false)
const healthPrompt = signal('Say hi briefly (one short sentence)')
const accountRefreshing = signal(false)
const accountData = signal<AccountInfo | null>(null)
const usageData = signal<UsageData | null>(null)
const usageLoading = signal(false)
const authStatus = signal<{ loggedIn: boolean; authMethod?: string } | null>(null)

function handleHealthRefreshClick() {
  showHealthModal.value = true
}

function handleRunHealthCheck() {
  showHealthModal.value = false
  healthRefreshing.value = true
  api
    .refreshHealth(healthPrompt.value, true)
    .catch(() => {})
    .finally(() => {
      healthRefreshing.value = false
    })
}

function handleAccountRefresh(force = true) {
  accountRefreshing.value = true
  api
    .getAccount(force)
    .then((data) => {
      accountData.value = data
    })
    .catch(() => {})
    .finally(() => {
      accountRefreshing.value = false
      accountCheckedAt.value = new Date()
    })
  usageLoading.value = true
  api
    .getUsage(force)
    .then((data) => {
      usageData.value = data
    })
    .catch(() => {})
    .finally(() => {
      usageLoading.value = false
    })
}

function handleLogin() {
  navigateToConsole()
  setTimeout(() => {
    const termEvent = new CustomEvent('terminal:command', {
      detail: { command: 'claude --dangerously-skip-permissions /login' },
    })
    window.dispatchEvent(termEvent)
  }, 500)
}

export function SingleDashboard() {
  const health = serverHealth.value
  const stats = serverStats.value
  const connected = isConnected.value
  const sessions = ptySessions.value

  // Sync usage from WS push
  const wsUsage = cachedUsage.value as UsageData | null
  if (wsUsage && wsUsage !== usageData.value) {
    usageData.value = wsUsage
  }

  const [_tick, setTick] = useState(0)
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 1000)
    return () => clearInterval(id)
  }, [])
  const liveUptime = (() => {
    if (!stats?.startedAt) return '—'
    const diff = Date.now() - new Date(stats.startedAt).getTime()
    const d = Math.floor(diff / 86400000)
    const h = Math.floor((diff % 86400000) / 3600000)
    const m = Math.floor((diff % 3600000) / 60000)
    const s = Math.floor((diff % 60000) / 1000)
    if (d > 0) return `${d}d ${h}h ${m}m`
    if (h > 0) return `${h}h ${m}m ${s}s`
    if (m > 0) return `${m}m ${s}s`
    return `${s}s`
  })()

  useEffect(() => {
    const fetchStatus = () =>
      api
        .getStatus()
        .then((data) => {
          serverStatus.value = data as any
        })
        .catch(() => {})
    // Merge live CPU%/RSS and the live message/token counters from the sessions REST endpoint into the WS-driven
    // ptySessions list (which carries the richer title/token fields), keyed by id.
    const refreshSessionStats = () =>
      api
        .getSessions()
        .then((data) => {
          type LiveStat = {
            id: string
            tokenId?: string
            engine?: string
            userName?: string
            lastActivityAt?: string
            mcpCallCount?: number
            cpuPercent?: number | null
            memBytes?: number | null
            attached?: boolean
            detachedForSec?: number | null
            userMessages?: number
            assistantMessages?: number
            contextTokens?: number
            totalTokens?: number
          }
          const byId = new Map((data.sessions as LiveStat[]).map((s) => [s.id, s]))
          ptySessions.value = ptySessions.value.map((p) => {
            const st = byId.get(p.id)
            // Also overlay attached/detachedForSec — the WS init payload omits them,
            // so the "⚠ detached" indicator only lights up via this REST merge.
            return st
              ? {
                  ...p,
                  tokenId: st.tokenId ?? p.tokenId,
                  engine: st.engine ?? p.engine,
                  userName: st.userName ?? p.userName,
                  lastActivityAt: st.lastActivityAt ?? p.lastActivityAt,
                  mcpCallCount: st.mcpCallCount ?? p.mcpCallCount,
                  cpuPercent: st.cpuPercent ?? null,
                  memBytes: st.memBytes ?? null,
                  attached: st.attached,
                  detachedForSec: st.detachedForSec ?? null,
                  // Live counters: the WS init is only a snapshot.
                  userMessages: st.userMessages ?? p.userMessages,
                  assistantMessages: st.assistantMessages ?? p.assistantMessages,
                  contextTokens: st.contextTokens ?? p.contextTokens,
                  totalTokens: st.totalTokens ?? p.totalTokens,
                }
              : p
          })
        })
        .catch(() => {})
    api
      .getRequests(100)
      .then((entries) => {
        capturedLog.value = entries
      })
      .catch(() => {})
    fetchStatus()
    refreshSessionStats()
    wsClient.onReconnect = fetchStatus
    // Poll server + per-session CPU/RAM every 5s. The server samples on its own
    // 2s background loop, so these endpoint reads just return the latest values.
    const statsTimer = setInterval(() => {
      fetchStatus()
      refreshSessionStats()
    }, 5000)

    // Load usage immediately (don't wait for auth)
    api
      .getUsage()
      .then((d) => {
        usageData.value = d
      })
      .catch(() => {})

    api
      .getAuthStatus()
      .then((data) => {
        authStatus.value = data
        if (data.loggedIn) {
          handleAccountRefresh(false)
          if (!serverHealth.value) {
            healthRefreshing.value = true
            api
              .refreshHealth(healthPrompt.value)
              .catch(() => {})
              .finally(() => {
                healthRefreshing.value = false
              })
          }
        }
      })
      .catch(() => {})

    if (cachedAccount.value && !accountData.value) {
      const ca = cachedAccount.value
      accountData.value = {
        email: ca.email || null,
        plan: ca.subscriptionType || null,
        organization: ca.organization || null,
      } as any
    }
    return () => {
      wsClient.onReconnect = null
      clearInterval(statsTimer)
    }
  }, [])

  const isLoggedIn = authStatus.value?.loggedIn !== false

  const timeAgo = (date: Date | null) => {
    if (!date) return ''
    const sec = Math.floor((Date.now() - date.getTime()) / 1000)
    if (sec < 60) return `${sec}s ago`
    const min = Math.floor(sec / 60)
    if (min < 60) return `${min}m ago`
    const h = Math.floor(min / 60)
    return `${h}h ${min % 60}m ago`
  }
  const accountAgo = timeAgo(accountCheckedAt.value)

  const sessionsStatus = sessions.length > 0 ? 'ok' : 'warning'
  // Derive a few useful breakdowns from the live PTY session list so the
  // status card carries weight even when a single tab is open.
  const distinctUsers = new Set(sessions.map(ptyUserKey)).size
  const totalInputs = sessions.reduce((acc, s) => acc + (s.userMessages || 0), 0)
  const totalMcpCalls = sessions.reduce((acc, s) => acc + (s.mcpCallCount || 0), 0)
  const lastActivityAt = sessions.reduce<number>((acc, s) => {
    const t = new Date(s.lastActivityAt).getTime()
    return t > acc ? t : acc
  }, 0)
  const lastActivityAgo =
    lastActivityAt > 0
      ? (() => {
          const sec = Math.max(0, Math.round((Date.now() - lastActivityAt) / 1000))
          if (sec < 60) return `${sec}s ago`
          if (sec < 3600) return `${Math.floor(sec / 60)}m ago`
          if (sec < 86400) return `${Math.floor(sec / 3600)}h ago`
          return `${Math.floor(sec / 86400)}d ago`
        })()
      : '—'
  const sessionsItems = [
    {
      label: 'Active Sessions',
      value: engineBreakdown(sessions),
      color: sessions.length > 0 ? 'var(--accent-green)' : 'var(--text-muted)',
    },
    ...(isLoggedIn
      ? [{ label: 'Auth', value: 'Authenticated', color: 'var(--accent-green)' }]
      : [{ label: 'Auth', value: 'Not authenticated', color: 'var(--accent-yellow)' }]),
    {
      label: 'Active Users',
      value: engineBreakdown(sessions, true),
      color: distinctUsers > 0 ? 'var(--accent-blue)' : 'var(--text-muted)',
    },
    { label: 'User msg', value: String(totalInputs), color: 'var(--text-secondary)' },
    { label: 'MCP Calls', value: String(totalMcpCalls), color: 'var(--text-secondary)' },
    { label: 'Last Activity', value: lastActivityAgo, color: 'var(--text-muted)' },
  ]

  const acct = accountData.value
  const loginEndsAt = acct?.loginExpiresAt ? Date.parse(acct.loginExpiresAt) : NaN
  const accountItems = !isLoggedIn
    ? [
        { label: 'Status', value: 'Not authenticated', color: 'var(--accent-red)' },
        { label: 'Auth', value: authStatus.value?.authMethod || 'none' },
      ]
    : acct
      ? [
          { label: 'Email', value: acct.email || '—' },
          { label: 'Plan', value: acct.plan || '—' },
          ...(acct.expiresAt
            ? [
                {
                  label: 'Token Expires',
                  value: new Date(acct.expiresAt).toLocaleString(),
                  color:
                    acct.expiresAt < Date.now()
                      ? 'var(--accent-red)'
                      : acct.expiresAt < Date.now() + 86400000
                        ? 'var(--accent-yellow)'
                        : undefined,
                },
              ]
            : []),
          ...(Number.isFinite(loginEndsAt)
            ? [
                {
                  label: 'Sign-in Ends',
                  value: new Date(loginEndsAt).toLocaleString(),
                  // The CLI starts warning 10 days before the stored sign-in ends.
                  color:
                    loginEndsAt < Date.now()
                      ? 'var(--accent-red)'
                      : loginEndsAt < Date.now() + 10 * 86400000
                        ? 'var(--accent-yellow)'
                        : undefined,
                },
              ]
            : []),
        ]
      : [{ label: 'Status', value: accountRefreshing.value ? 'Checking...' : 'Unavailable' }]

  return (
    <div class={styles.dashboard}>
      {/* Status Cards */}
      <div class={styles.statusRow}>
        <StatusCard
          icon="fa-solid fa-terminal"
          iconColor="var(--accent-blue)"
          title="Sessions"
          status={sessionsStatus}
          items={sessionsItems}
        />
        <StatusCard
          icon="fa-solid fa-user"
          iconColor="var(--accent-purple)"
          title="Account"
          status={!isLoggedIn ? 'error' : acct ? 'ok' : 'warning'}
          items={accountItems}
          action={
            isLoggedIn
              ? {
                  icon: 'fa-solid fa-rotate',
                  label: 'Refresh Account',
                  onClick: () => handleAccountRefresh(true),
                  loading: accountRefreshing.value || usageLoading.value,
                  hint: accountAgo,
                }
              : { icon: 'fa-solid fa-right-to-bracket', label: 'Login', onClick: handleLogin }
          }
        >
          {isLoggedIn && <UsageBars usage={usageData.value} loading={usageLoading.value} />}
          {!isLoggedIn && (
            <div style={{ padding: '12px 0 4px', textAlign: 'center' }}>
              <button onClick={handleLogin} class={styles.loginBtn}>
                <i class="fa-solid fa-right-to-bracket" /> Login with Claude
              </button>
            </div>
          )}
        </StatusCard>
        <StatusCard
          icon="fa-solid fa-server"
          iconColor="var(--accent-green)"
          title="Server"
          status={connected ? 'ok' : 'error'}
          items={(() => {
            const status = serverStatus.value as any
            const dbInfo = status?.db as { type?: string; version?: string | null; sizeBytes?: number } | undefined
            const fmtSize = (n: number): string => {
              if (n >= 1024 * 1024 * 1024) return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`
              if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
              if (n >= 1024) return `${(n / 1024).toFixed(0)} KB`
              return `${n} B`
            }
            const dbValue = dbInfo
              ? `${dbInfo.type || 'duckdb'}${dbInfo.version ? ` ${dbInfo.version}` : ''}${dbInfo.sizeBytes ? ` · ${fmtSize(dbInfo.sizeBytes)}` : ''}`
              : '—'
            const startedStr = stats?.startedAt ? new Date(stats.startedAt).toLocaleString() : '—'
            return [
              { label: 'Version', value: status?.version || '—', color: 'var(--accent-purple)' },
              {
                label: 'WebSocket',
                value: connected ? 'Connected' : 'Disconnected',
                color: connected ? 'var(--accent-green)' : 'var(--accent-red)',
              },
              { label: 'Started / Uptime', value: `${startedStr} / ${liveUptime}` },
              { label: 'DB', value: dbValue, color: 'var(--accent-blue)' },
              {
                label: 'OS',
                value: (() => {
                  const sys = status?.system as { cpuPercent?: number; memPercent?: number } | undefined
                  const base = status?.os || '—'
                  return sys ? `${base}, CPU ${sys.cpuPercent ?? 0}%, RAM ${sys.memPercent ?? 0}%` : base
                })(),
              },
              { label: 'Node', value: status?.nodeVersion || '—' },
              (() => {
                const row = codexStatusRow(status?.codex as CodexStatus | undefined)
                return { label: 'Codex', value: row.text, color: row.ok ? undefined : 'var(--accent-red)' }
              })(),
            ]
          })()}
        />
        {serverStatus.value?.codex?.available && (
          <CodexAccountCard
            version={serverStatus.value.codex.version}
            activeSessions={sessions.filter((s) => s.engine === 'codex').length}
          />
        )}
      </div>

      <ActivePtySessions sessions={sessions} />

      {/* API Errors — captured from CLI retry log lines via session-io regex. */}
      <ApiErrorsCard />

      {/* Usage Chart — driven by jsonl_events × session_tokens (deleted=0),
          same model as Stats grid below. */}
      <UsageChart />

      {/* Stats grid: "All tokens" first, then one card per user/token */}
      <TokensStatsGrid />
      <McpUsageReport />

      {/* Selected token → modal with session list (titles, counts, download).
          Count is wired up to the modal title via onCountChange so the inner
          subheader is not needed. */}
      <SelectedTokenModal />

      {/* Response Detail Modal */}
      <Modal
        isOpen={showResponseModal.value}
        onClose={() => {
          showResponseModal.value = false
        }}
        title="Health Check Response"
        size="sm"
      >
        <pre
          style={{
            background: 'var(--bg-tertiary)',
            border: '1px solid var(--border-primary)',
            borderRadius: '6px',
            padding: '14px',
            color: 'var(--text-primary)',
            fontFamily: 'var(--font-mono)',
            fontSize: '13px',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
            margin: 0,
            lineHeight: '1.5',
          }}
        >
          {health?.testResponse || health?.error || '—'}
        </pre>
      </Modal>

      {/* Errors Modal */}
      <Modal
        isOpen={showErrorsModal.value}
        onClose={() => {
          showErrorsModal.value = false
        }}
        title={`Errors (${errorLog.value.length})`}
        size="lg"
      >
        <ErrorLog
          errors={errorLog.value}
          onClear={() => {
            clearErrors()
            showErrorsModal.value = false
          }}
        />
      </Modal>

      {/* Health Check Modal */}
      <Modal
        isOpen={showHealthModal.value}
        onClose={() => {
          showHealthModal.value = false
        }}
        title="Health Check"
        size="sm"
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <label style={{ fontSize: '12px', color: 'var(--text-muted)' }}>
            Test prompt (sent to /claude-code/v1/messages)
          </label>
          <textarea
            value={healthPrompt.value}
            onInput={(e) => {
              healthPrompt.value = (e.target as HTMLTextAreaElement).value
            }}
            rows={3}
            style={{
              background: 'var(--bg-tertiary)',
              border: '1px solid var(--border-primary)',
              borderRadius: '6px',
              padding: '10px',
              color: 'var(--text-primary)',
              fontFamily: 'var(--font-mono)',
              fontSize: '13px',
              resize: 'vertical',
            }}
          />
          <button
            onClick={handleRunHealthCheck}
            style={{
              background: 'var(--accent-blue)',
              color: '#fff',
              border: 'none',
              borderRadius: '6px',
              padding: '8px 16px',
              cursor: 'pointer',
              fontWeight: '600',
              fontSize: '13px',
            }}
          >
            <i class="fa-solid fa-play" style={{ marginRight: '6px' }} />
            Run Health Check
          </button>
        </div>
      </Modal>
    </div>
  )
}
