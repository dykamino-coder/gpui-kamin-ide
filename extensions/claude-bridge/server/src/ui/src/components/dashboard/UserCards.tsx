import { signal } from '@preact/signals'
import { useEffect } from 'preact/hooks'
import { Card, SectionHeader } from '../shared'
import { userColor } from '../../utils/user-colors'
import { api } from '../../services/api-client'
import type { UserSummary } from '../../services/api-client'
import { selectedTokenId, selectTokenId } from '../../signals/ui'
import styles from './UserCards.module.css'

const users = signal<UserSummary[]>([])
const loading = signal(false)

function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function fmtCost(n: number): string {
  if (n >= 100) return `$${n.toFixed(0)}`
  if (n >= 1) return `$${n.toFixed(2)}`
  if (n > 0) return `$${n.toFixed(3)}`
  return '$0'
}

function timeAgo(iso: string | null): string {
  if (!iso) return '—'
  const sec = Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  if (sec < 60) return `${sec}s ago`
  const min = Math.floor(sec / 60)
  if (min < 60) return `${min}m ago`
  const h = Math.floor(min / 60)
  if (h < 24) return `${h}h ago`
  const d = Math.floor(h / 24)
  return `${d}d ago`
}

export async function fetchUsers() {
  loading.value = true
  try {
    users.value = await api.getUsers()
  } catch {}
  loading.value = false
}

export function UserCards() {
  useEffect(() => {
    fetchUsers()
    const id = setInterval(fetchUsers, 30000)
    return () => clearInterval(id)
  }, [])

  const data = users.value
  const isLoading = loading.value

  if (isLoading && data.length === 0) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
        <SectionHeader title="Users" icon="fa-solid fa-users" />
        <Card>
          <div class={styles.loading}><i class="fa-solid fa-spinner fa-spin" /> Loading...</div>
        </Card>
      </div>
    )
  }

  if (data.length === 0) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
        <SectionHeader title="Users" icon="fa-solid fa-users" />
        <Card>
          <div class={styles.empty}>No user data yet</div>
        </Card>
      </div>
    )
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
      <SectionHeader title="Users" icon="fa-solid fa-users" subtitle={`${data.length} users`} />
      <div class={styles.grid}>
        {data.map((u) => {
          const color = userColor(u.userName)
          const totalTokens = u.tokens.input + u.tokens.output + u.tokens.cacheRead + u.tokens.cacheCreation
          const isSelected = selectedTokenId.value === u.userName
          return (
            <Card
              key={u.userName}
              className={styles.userCard}
              clickable
              hoverable
              selected={isSelected}
              onClick={() => selectTokenId(isSelected ? null : u.userName)}
            >
              <div class={styles.cardHeader}>
                <div class={styles.avatar} style={{ background: color }}>
                  {u.userName.charAt(0).toUpperCase()}
                </div>
                <div class={styles.nameWrap}>
                  <span class={styles.userName}>{u.userName}</span>
                  <span class={styles.lastSeen}>{timeAgo(u.lastSeen)}</span>
                </div>
                {u.activeSessions > 0 && (
                  <span class={styles.activeBadge}>
                    <span class={styles.activeDot} /> {u.activeSessions}
                  </span>
                )}
              </div>
              <div class={styles.statsRow}>
                <div class={styles.stat}>
                  <span class={styles.statValue} style={{ color: 'var(--accent-blue)' }}>{u.totalInputs}</span>
                  <span class={styles.statLabel}>Inputs</span>
                </div>
                <div class={styles.stat}>
                  <span class={styles.statValue} style={{ color: 'var(--accent-purple)' }}>{fmtTokens(totalTokens)}</span>
                  <span class={styles.statLabel}>Tokens</span>
                </div>
                <div class={styles.stat}>
                  <span class={styles.statValue} style={{ color: 'var(--accent-green)' }}>{fmtCost(u.cost)}</span>
                  <span class={styles.statLabel}>Cost</span>
                </div>
              </div>
              {totalTokens > 0 && (
                <div class={styles.tokenBar}>
                  <div class={styles.tokenSegment} style={{ flex: u.tokens.input, background: 'var(--accent-blue)' }} title={`Input: ${fmtTokens(u.tokens.input)}`} />
                  <div class={styles.tokenSegment} style={{ flex: u.tokens.output, background: 'var(--accent-purple)' }} title={`Output: ${fmtTokens(u.tokens.output)}`} />
                  <div class={styles.tokenSegment} style={{ flex: u.tokens.cacheRead, background: 'var(--accent-green)' }} title={`Cache Read: ${fmtTokens(u.tokens.cacheRead)}`} />
                  <div class={styles.tokenSegment} style={{ flex: u.tokens.cacheCreation, background: 'var(--accent-yellow)' }} title={`Cache Write: ${fmtTokens(u.tokens.cacheCreation)}`} />
                </div>
              )}
              <div class={styles.tokenLegend}>
                <span><i class={styles.dot} style={{ background: 'var(--accent-blue)' }} /> In {fmtTokens(u.tokens.input)}</span>
                <span><i class={styles.dot} style={{ background: 'var(--accent-purple)' }} /> Out {fmtTokens(u.tokens.output)}</span>
                <span><i class={styles.dot} style={{ background: 'var(--accent-green)' }} /> Cache R {fmtTokens(u.tokens.cacheRead)}</span>
                <span><i class={styles.dot} style={{ background: 'var(--accent-yellow)' }} /> Cache W {fmtTokens(u.tokens.cacheCreation)}</span>
              </div>
            </Card>
          )
        })}
      </div>
    </div>
  )
}
