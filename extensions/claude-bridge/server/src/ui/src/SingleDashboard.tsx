import { useEffect, useState } from 'preact/compat'
import { signal } from '@preact/signals'
import { StatusCard } from './components/dashboard/StatusCard'
import { ErrorLog } from './components/errors/ErrorLog'
import { TokenSessionsList, TokensStatsGrid } from './components/dashboard/StatsCard'
import { selectedTokenId, selectTokenId } from './signals/ui'
import { SectionHeader, Card, Modal } from './components/shared'
import { serverHealth, serverStats, serverStatus, isConnected, ptySessions, cachedAccount, accountCheckedAt, cachedUsage } from './signals/server'
import { errorLog, clearErrors, capturedLog } from './signals/requests'
import { api } from './services/api-client'
import { wsClient } from './services/ws-client'
import type { UsageData, AccountInfo } from './services/api-client'
import { UsageBars } from './components/dashboard/UsageBars'
import { UsageChart } from './components/dashboard/UsageChart'
import { ApiErrorsCard } from './components/dashboard/ApiErrorsCard'

import { navigateToConsole } from './router'
import styles from './SingleDashboard.module.css'

const showHealthModal = signal(false)
const showResponseModal = signal(false)
const showErrorsModal = signal(false)
const selectedTokenSessionCount = signal<number | null>(null)

function SelectedTokenModal() {
  const id = selectedTokenId.value
  const count = selectedTokenSessionCount.value
  const title = id
    ? (count != null ? `${id} — Sessions (${count})` : `${id} — Sessions`)
    : 'Sessions'
  return (
    <Modal
      isOpen={!!id}
      onClose={() => { selectedTokenSessionCount.value = null; selectTokenId(null) }}
      title={title}
      size="lg"
    >
      {id && <TokenSessionsList tokenId={id} onCountChange={(n) => { selectedTokenSessionCount.value = n }} />}
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
  api.refreshHealth(healthPrompt.value, true)
    .catch(() => {})
    .finally(() => { healthRefreshing.value = false })
}

function handleAccountRefresh(force = true) {
  accountRefreshing.value = true
  api.getAccount(force).then((data) => { accountData.value = data }).catch(() => {}).finally(() => { accountRefreshing.value = false; accountCheckedAt.value = new Date() })
  usageLoading.value = true
  api.getUsage(force).then((data) => { usageData.value = data }).catch(() => {}).finally(() => { usageLoading.value = false })
}

function handleLogin() {
  navigateToConsole()
  setTimeout(() => {
    const termEvent = new CustomEvent('terminal:command', { detail: { command: 'claude --dangerously-skip-permissions /login' } })
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
    const id = setInterval(() => setTick(t => t + 1), 1000)
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
    const fetchStatus = () => api.getStatus().then((data) => { serverStatus.value = data as any }).catch(() => {})
    // Merge live CPU%/RSS from the sessions REST endpoint into the WS-driven
    // ptySessions list (which carries the richer title/token fields), keyed by id.
    const refreshSessionStats = () => api.getSessions().then((data) => {
      type LiveStat = { id: string; cpuPercent?: number | null; memBytes?: number | null; attached?: boolean; detachedForSec?: number | null }
      const byId = new Map((data.sessions as LiveStat[]).map((s) => [s.id, s]))
      ptySessions.value = ptySessions.value.map((p) => {
        const st = byId.get(p.id)
        // Also overlay attached/detachedForSec — the WS init payload omits them,
        // so the "⚠ detached" indicator only lights up via this REST merge.
        return st ? { ...p, cpuPercent: st.cpuPercent ?? null, memBytes: st.memBytes ?? null, attached: st.attached, detachedForSec: st.detachedForSec ?? null } : p
      })
    }).catch(() => {})
    api.getRequests(100).then((entries) => { capturedLog.value = entries }).catch(() => {})
    fetchStatus()
    refreshSessionStats()
    wsClient.onReconnect = fetchStatus
    // Poll server + per-session CPU/RAM every 5s. The server samples on its own
    // 2s background loop, so these endpoint reads just return the latest values.
    const statsTimer = setInterval(() => { fetchStatus(); refreshSessionStats() }, 5000)

    // Load usage immediately (don't wait for auth)
    api.getUsage().then((d) => { usageData.value = d }).catch(() => {})

    api.getAuthStatus().then((data) => {
      authStatus.value = data
      if (data.loggedIn) {
        handleAccountRefresh(false)
        if (!serverHealth.value) {
          healthRefreshing.value = true
          api.refreshHealth(healthPrompt.value)
            .catch(() => {})
            .finally(() => { healthRefreshing.value = false })
        }
      }
    }).catch(() => {})

    if (cachedAccount.value && !accountData.value) {
      const ca = cachedAccount.value
      accountData.value = { email: ca.email || null, plan: ca.subscriptionType || null, organization: ca.organization || null } as any
    }
    return () => { wsClient.onReconnect = null; clearInterval(statsTimer) }
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
  const distinctUsers = new Set(sessions.map(s => s.userName).filter(Boolean)).size
  const totalInputs = sessions.reduce((acc, s) => acc + (s.userMessages || 0), 0)
  const totalMcpCalls = sessions.reduce((acc, s) => acc + (s.mcpCallCount || 0), 0)
  const lastActivityAt = sessions.reduce<number>((acc, s) => {
    const t = new Date(s.lastActivityAt).getTime()
    return t > acc ? t : acc
  }, 0)
  const lastActivityAgo = lastActivityAt > 0
    ? (() => {
        const sec = Math.max(0, Math.round((Date.now() - lastActivityAt) / 1000))
        if (sec < 60) return `${sec}s ago`
        if (sec < 3600) return `${Math.floor(sec / 60)}m ago`
        if (sec < 86400) return `${Math.floor(sec / 3600)}h ago`
        return `${Math.floor(sec / 86400)}d ago`
      })()
    : '—'
  const sessionsItems = [
    { label: 'Active Sessions', value: String(sessions.length), color: sessions.length > 0 ? 'var(--accent-green)' : 'var(--text-muted)' },
    ...(isLoggedIn ? [
      { label: 'Auth', value: 'Authenticated', color: 'var(--accent-green)' },
    ] : [
      { label: 'Auth', value: 'Not authenticated', color: 'var(--accent-yellow)' },
    ]),
    { label: 'Active Users', value: String(distinctUsers), color: distinctUsers > 0 ? 'var(--accent-blue)' : 'var(--text-muted)' },
    { label: 'User msg', value: String(totalInputs), color: 'var(--text-secondary)' },
    { label: 'MCP Calls', value: String(totalMcpCalls), color: 'var(--text-secondary)' },
    { label: 'Last Activity', value: lastActivityAgo, color: 'var(--text-muted)' },
  ]

  const acct = accountData.value
  const accountItems = !isLoggedIn
    ? [
        { label: 'Status', value: 'Not authenticated', color: 'var(--accent-red)' },
        { label: 'Auth', value: authStatus.value?.authMethod || 'none' },
      ]
    : acct
    ? [
        { label: 'Email', value: acct.email || '—' },
        { label: 'Plan', value: acct.plan || '—' },
        ...(acct.expiresAt ? [{
          label: 'Token Expires',
          value: new Date(acct.expiresAt).toLocaleString(),
          color: acct.expiresAt < Date.now() ? 'var(--accent-red)' : acct.expiresAt < Date.now() + 86400000 ? 'var(--accent-yellow)' : undefined,
        }] : []),
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
          action={isLoggedIn
            ? { icon: 'fa-solid fa-rotate', label: 'Refresh Account', onClick: () => handleAccountRefresh(true), loading: accountRefreshing.value || usageLoading.value, hint: accountAgo }
            : { icon: 'fa-solid fa-right-to-bracket', label: 'Login', onClick: handleLogin }
          }
        >
          {isLoggedIn && <UsageBars usage={usageData.value} loading={usageLoading.value} />}
          {!isLoggedIn && (
            <div style={{ padding: '12px 0 4px', textAlign: 'center' }}>
              <button
                onClick={handleLogin}
                class={styles.loginBtn}
              >
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
              { label: 'WebSocket', value: connected ? 'Connected' : 'Disconnected', color: connected ? 'var(--accent-green)' : 'var(--accent-red)' },
              { label: 'Started / Uptime', value: `${startedStr} / ${liveUptime}` },
              { label: 'DB', value: dbValue, color: 'var(--accent-blue)' },
              { label: 'OS', value: (() => {
                const sys = status?.system as { cpuPercent?: number; memPercent?: number } | undefined
                const base = status?.os || '—'
                return sys ? `${base}, CPU ${sys.cpuPercent ?? 0}%, RAM ${sys.memPercent ?? 0}%` : base
              })() },
              { label: 'Node', value: status?.nodeVersion || '—' },
            ]
          })()}
        />
      </div>

      {/* Active PTY Sessions — header lives inside the card, matching the
          Usage chart layout above. */}
      <Card>
        <div style={{ padding: '14px 16px 0' }}>
          <SectionHeader title="Active PTY Sessions" icon="fa-solid fa-terminal" subtitle={sessions.length > 0 ? `${sessions.length} active` : undefined} />
        </div>
        {sessions.length === 0 ? (
          <div style={{ padding: '24px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '13px' }}>
            No active sessions
          </div>
        ) : (
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '13px' }}>
              <thead>
                <tr style={{ borderBottom: '1px solid var(--border-primary)' }}>
                  <th style={{ textAlign: 'left', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}>Title / ID</th>
                  <th style={{ textAlign: 'left', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}>User</th>
                  <th style={{ textAlign: 'left', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}>Folder</th>
                  <th style={{ textAlign: 'left', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}>State / MCP</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}>User</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}>Asst</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}>MCP</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }} title="Real context window at last turn">Context</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }} title="Cumulative API throughput">Total</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }} title="Live CPU% over the session's whole process tree (PTY shell → claude → MCP children). Per-core, like top.">CPU</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }} title="Live resident memory (RSS) over the session's whole process tree">Mem</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}>Duration</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}>Idle</th>
                  <th style={{ textAlign: 'right', padding: '8px 12px', color: 'var(--text-muted)', fontWeight: 500 }}></th>
                </tr>
              </thead>
              <tbody>
                {sessions.map((s) => {
                  const folder = s.cwd ? s.cwd.split(/[/\\]/).pop() || s.cwd : '—'
                  // Clamp to 0 — server clock may be ahead of the browser
                  // (e.g. UTC container vs local TZ skew on a laptop with
                  // drifted system clock), which would otherwise produce
                  // negative "Duration / Idle" cells.
                  const durSec = Math.max(0, Math.round((Date.now() - new Date(s.createdAt).getTime()) / 1000))
                  const idleSec = Math.max(0, Math.round((Date.now() - new Date(s.lastActivityAt).getTime()) / 1000))
                  const fmtDur = (sec: number) => {
                    if (sec < 60) return `${sec}s`
                    const m = Math.floor(sec / 60), ss = sec % 60
                    if (m < 60) return `${m}m ${ss}s`
                    const h = Math.floor(m / 60)
                    return `${h}h ${m % 60}m`
                  }
                  const fmtTok = (n: number) => {
                    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
                    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
                    return String(n || 0)
                  }
                  const fmtMem = (bytes: number) => {
                    if (bytes >= 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024 / 1024).toFixed(1)}G`
                    if (bytes >= 1024 * 1024) return `${Math.round(bytes / 1024 / 1024)}M`
                    return `${Math.round(bytes / 1024)}K`
                  }
                  // Per-core CPU%: colour hot sessions (a pegged core = 100%).
                  const cpuPct = typeof s.cpuPercent === 'number' ? s.cpuPercent : null
                  const cpuColor = cpuPct === null ? 'var(--text-muted)' : cpuPct >= 80 ? 'var(--accent-red)' : cpuPct >= 30 ? 'var(--accent-yellow)' : 'var(--text-secondary)'
                  const stateColor = s.state === 'running' ? 'var(--accent-green)' : s.state === 'exiting' ? 'var(--accent-yellow)' : 'var(--text-muted)'
                  const mcpOk = s.mcpInitialized === true
                  const mcpErr = s.mcpLastError
                  const mcpColor = mcpOk ? 'var(--accent-green)' : mcpErr ? 'var(--accent-red)' : 'var(--accent-yellow)'
                  const mcpLabel = mcpOk ? 'OK' : mcpErr ? 'Error' : 'Pending'
                  const titleText = s.sessionTitle || s.id.slice(0, 8) + '…'
                  return (
                    <tr key={s.id} style={{ borderBottom: '1px solid var(--border-secondary)' }}>
                      <td style={{ padding: '8px 12px', maxWidth: 240, overflow: 'hidden' }} title={s.sessionTitle || s.id}>
                        <div style={{ color: 'var(--text-primary)', fontWeight: s.sessionTitle ? 600 : 400, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                          {titleText}
                        </div>
                        <div style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }} title={s.id}>
                          {s.id.slice(0, 8)}
                        </div>
                      </td>
                      <td style={{ padding: '8px 12px', color: 'var(--text-primary)' }}>{s.userName || '—'}</td>
                      <td style={{ padding: '8px 12px', color: 'var(--accent-blue)', maxWidth: '200px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={s.cwd}>{folder}</td>
                      <td style={{ padding: '8px 12px' }} title={mcpErr || ''}>
                        <div style={{ color: stateColor, fontWeight: 500, fontSize: '12px' }}>{s.state}</div>
                        <div style={{ color: mcpColor, fontSize: '10px' }}>MCP: {mcpLabel}</div>
                        {s.attached === false ? (
                          <div style={{ color: 'var(--accent-yellow)', fontSize: '10px' }} title="No client is connected — the session survives a client drop and is waiting out the detach-grace window before the reaper kills it">
                            ⚠ detached{typeof s.detachedForSec === 'number' ? ` · ${fmtDur(s.detachedForSec)}` : ''}
                          </div>
                        ) : (
                          <div style={{ color: 'var(--accent-green)', fontSize: '10px' }} title="A client is connected to this session">● client</div>
                        )}
                      </td>
                      <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--accent-blue)' }} title="User messages from JSONL">{s.userMessages ?? 0}</td>
                      <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)' }} title="Assistant messages from JSONL">{s.assistantMessages ?? 0}</td>
                      <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)' }} title="MCP tool calls">{s.mcpCallCount}</td>
                      <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--accent-purple)' }} title="Real context window at last assistant turn">{fmtTok(s.contextTokens ?? 0)}</td>
                      <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)' }} title="Cumulative API throughput (input + cache + output)">{fmtTok(s.totalTokens ?? 0)}</td>
                      <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'var(--font-mono)', color: cpuColor }} title="Live CPU% over the session's process tree (per-core)">{cpuPct === null ? '—' : `${cpuPct}%`}</td>
                      <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)' }} title="Live RSS over the session's process tree">{typeof s.memBytes === 'number' ? fmtMem(s.memBytes) : '—'}</td>
                      <td style={{ padding: '8px 12px', textAlign: 'right', color: 'var(--text-muted)' }}>{fmtDur(durSec)}</td>
                      <td style={{ padding: '8px 12px', textAlign: 'right', color: idleSec > 300 ? 'var(--accent-yellow)' : 'var(--text-muted)' }}>{fmtDur(idleSec)}</td>
                      <td style={{ padding: '8px 12px', textAlign: 'right' }}>
                        <button
                          type="button"
                          title="Terminate this session — kills the claude CLI process on the server"
                          onClick={() => {
                            if (!confirm(`Kill session "${titleText}"?\nThis terminates the claude process on the server.`)) return
                            void api.killSession(s.id).catch(() => { /* row drops on the WS destroy event; ignore */ })
                          }}
                          style={{
                            padding: '3px 10px', fontSize: '11px', fontWeight: 600, cursor: 'pointer',
                            color: 'var(--accent-red)', background: 'transparent',
                            border: '1px solid var(--accent-red)', borderRadius: '5px',
                          }}
                        >
                          Kill
                        </button>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
        )}
      </Card>

      {/* API Errors — captured from CLI retry log lines via session-io regex. */}
      <ApiErrorsCard />

      {/* Usage Chart — driven by jsonl_events × session_tokens (deleted=0),
          same model as Stats grid below. */}
      <UsageChart />

      {/* Stats grid: "All tokens" first, then one card per user/token */}
      <TokensStatsGrid />

      {/* Selected token → modal with session list (titles, counts, download).
          Count is wired up to the modal title via onCountChange so the inner
          subheader is not needed. */}
      <SelectedTokenModal />

      {/* Response Detail Modal */}
      <Modal
        isOpen={showResponseModal.value}
        onClose={() => { showResponseModal.value = false }}
        title="Health Check Response"
        size="sm"
      >
        <pre style={{
          background: 'var(--bg-tertiary)', border: '1px solid var(--border-primary)',
          borderRadius: '6px', padding: '14px', color: 'var(--text-primary)',
          fontFamily: 'var(--font-mono)', fontSize: '13px', whiteSpace: 'pre-wrap',
          wordBreak: 'break-word', margin: 0, lineHeight: '1.5',
        }}>
          {health?.testResponse || health?.error || '—'}
        </pre>
      </Modal>

      {/* Errors Modal */}
      <Modal
        isOpen={showErrorsModal.value}
        onClose={() => { showErrorsModal.value = false }}
        title={`Errors (${errorLog.value.length})`}
        size="lg"
      >
        <ErrorLog errors={errorLog.value} onClear={() => { clearErrors(); showErrorsModal.value = false }} />
      </Modal>

      {/* Health Check Modal */}
      <Modal
        isOpen={showHealthModal.value}
        onClose={() => { showHealthModal.value = false }}
        title="Health Check"
        size="sm"
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <label style={{ fontSize: '12px', color: 'var(--text-muted)' }}>Test prompt (sent to /claude-code/v1/messages)</label>
          <textarea
            value={healthPrompt.value}
            onInput={(e) => { healthPrompt.value = (e.target as HTMLTextAreaElement).value }}
            rows={3}
            style={{
              background: 'var(--bg-tertiary)', border: '1px solid var(--border-primary)',
              borderRadius: '6px', padding: '10px', color: 'var(--text-primary)',
              fontFamily: 'var(--font-mono)', fontSize: '13px', resize: 'vertical',
            }}
          />
          <button
            onClick={handleRunHealthCheck}
            style={{
              background: 'var(--accent-blue)', color: '#fff', border: 'none',
              borderRadius: '6px', padding: '8px 16px', cursor: 'pointer',
              fontWeight: '600', fontSize: '13px',
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
