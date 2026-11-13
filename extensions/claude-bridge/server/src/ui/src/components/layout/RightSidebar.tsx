import { memo } from 'preact/compat'
import { useEffect, useMemo, useState } from 'preact/hooks'
import { selectTokenId } from '../../signals/ui'
import { formatTokens, formatCost } from '../../utils/formatters'
import { userColor } from '../../utils/user-colors'
import { api } from '../../services/api-client'
import type { UserSummary } from '../../services/api-client'
import { refreshUsers, statsBuilding, subscribeUserStats, users } from '../../signals/users-stats'
import styles from './RightSidebar.module.css'

// Explicit, labelled sort. The order is frozen between data refreshes so rows
// never jump under the pointer; new users are appended, and a "Re-sort" button
// appears when fresh numbers would order the list differently.
type SortMode = 'active' | 'tokens7d' | 'tokens30d' | 'tokensAll' | 'name'
const SORT_LABELS: Record<SortMode, string> = {
  active: 'Active now',
  tokens7d: 'Tokens · 7 days',
  tokens30d: 'Tokens · 30 days',
  tokensAll: 'Tokens · all time',
  name: 'Name',
}
const SORT_KEY = 'dashboard.users.sort'

function loadSort(): SortMode {
  try {
    const v = localStorage.getItem(SORT_KEY)
    if (v && v in SORT_LABELS) return v as SortMode
  } catch {
    /* storage blocked */
  }
  return 'active'
}

function saveSort(mode: SortMode): void {
  try {
    localStorage.setItem(SORT_KEY, mode)
  } catch {
    /* storage blocked */
  }
}

const totalTokens = (u: UserSummary) => u.tokens.input + u.tokens.output + u.tokens.cacheRead + u.tokens.cacheCreation

function compareFor(mode: SortMode): (a: UserSummary, b: UserSummary) => number {
  const byName = (a: UserSummary, b: UserSummary) => a.userName.localeCompare(b.userName)
  const desc = (f: (u: UserSummary) => number) => (a: UserSummary, b: UserSummary) => f(b) - f(a) || byName(a, b)
  switch (mode) {
    case 'active':
      return (a, b) =>
        b.activeSessions - a.activeSessions || (b.lastSeen ?? '').localeCompare(a.lastSeen ?? '') || byName(a, b)
    case 'tokens7d':
      return desc((u) => u.periodTokens?.d7 ?? 0)
    case 'tokens30d':
      return desc((u) => u.periodTokens?.d30 ?? 0)
    case 'tokensAll':
      return desc(totalTokens)
    default:
      return byName
  }
}

function sortedNames(list: UserSummary[], mode: SortMode): string[] {
  return [...list].sort(compareFor(mode)).map((u) => u.userName)
}

function userInitial(name: string): string {
  return name.charAt(0).toUpperCase()
}

function timeAgo(iso: string | null): string {
  if (!iso) return ''
  const sec = Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  if (sec < 60) return `${sec}s ago`
  const min = Math.floor(sec / 60)
  if (min < 60) return `${min}m ago`
  const h = Math.floor(min / 60)
  if (h < 24) return `${h}h ago`
  const d = Math.floor(h / 24)
  return `${d}d ago`
}

const UserRow = memo(function UserRow({ u, mode }: { u: UserSummary; mode: SortMode }) {
  const total = totalTokens(u)
  const period = mode === 'tokens7d' ? u.periodTokens?.d7 : mode === 'tokens30d' ? u.periodTokens?.d30 : undefined
  const handleDelete = (e: MouseEvent) => {
    e.stopPropagation()
    if (!confirm(`Delete all requests from "${u.userName}"?`)) return
    api
      .deleteByFilter({ userName: u.userName })
      .then(() => refreshUsers())
      .catch(() => {})
  }
  return (
    <button class={styles.userCard} onClick={() => selectTokenId(u.userName)} title={u.userName}>
      <div class={styles.avatar} style={{ background: userColor(u.userName) }}>
        {userInitial(u.userName)}
      </div>
      <div class={styles.userInfo}>
        <div class={styles.userNameRow}>
          <span class={styles.userName}>{u.userName.toUpperCase()}</span>
          {u.activeSessions > 0 && (
            <span class={styles.activeBadge}>
              <span class={styles.activeDot} /> {u.activeSessions}
            </span>
          )}
        </div>
        <div class={styles.userMeta}>
          <span class={styles.userMetaItem} title="User inputs">
            <i class="fa-solid fa-keyboard" />
            {u.totalInputs}
          </span>
          {period !== undefined ? (
            <span class={styles.userMetaItem} title={`${SORT_LABELS[mode]}: ${period.toLocaleString()}`}>
              <i class="fa-solid fa-coins" />
              {formatTokens(period)}
            </span>
          ) : (
            total > 0 && (
              <span class={styles.userMetaItem} title={`Total tokens: ${total.toLocaleString()}`}>
                <i class="fa-solid fa-coins" />
                {formatTokens(total)}
              </span>
            )
          )}
          {u.contextTokens > 0 && (
            <span
              class={styles.userMetaItem}
              title={`Real conversation context: ${u.contextTokens.toLocaleString()}`}
              style={{ color: 'var(--accent-purple)' }}
            >
              <i class="fa-solid fa-layer-group" />
              {formatTokens(u.contextTokens)}
            </span>
          )}
          {u.cost > 0 && (
            <span class={styles.userMetaItem} title={`Estimated USD cost`} style={{ color: 'var(--accent-green)' }}>
              {formatCost(u.cost)}
            </span>
          )}
        </div>
        {u.lastSeen && <div class={styles.userTime}>{timeAgo(u.lastSeen)}</div>}
      </div>
      <button class={styles.deleteBtn} onClick={(e: any) => handleDelete(e)} title={`Delete ${u.userName} requests`}>
        <i class="fa-solid fa-trash-can" />
      </button>
    </button>
  )
})

export function RightSidebar() {
  useEffect(() => subscribeUserStats(), [])
  const data = users.value
  const [mode, setMode] = useState<SortMode>(loadSort)
  const [order, setOrder] = useState<string[]>([])

  const byName = useMemo(() => new Map(data.map((u) => [u.userName, u])), [data])
  // Keep the frozen order; drop vanished users, append new ones in sort order.
  const shown = useMemo(() => {
    const kept = order.filter((n) => byName.has(n))
    const known = new Set(kept)
    const added = sortedNames(
      data.filter((u) => !known.has(u.userName)),
      mode,
    )
    return [...kept, ...added]
  }, [order, byName, data, mode])
  const fresh = useMemo(() => sortedNames(data, mode), [data, mode])
  const stale = shown.length > 0 && shown.some((n, i) => n !== fresh[i])

  // First data and every explicit mode change apply the sort immediately.
  useEffect(() => {
    if (order.length === 0 && data.length > 0) setOrder(fresh)
  }, [data.length])

  // The roster served during the one-time statistics build carries no numbers:
  // apply the sort again once the real numbers arrive.
  const building = statsBuilding.value
  const [sawBuild, setSawBuild] = useState(false)
  useEffect(() => {
    if (building != null) setSawBuild(true)
    else if (sawBuild && data.length > 0) {
      setSawBuild(false)
      setOrder(fresh)
    }
  }, [building, data])

  const pickMode = (next: SortMode) => {
    setMode(next)
    saveSort(next)
    setOrder(sortedNames(data, next))
  }

  return (
    <aside class={styles.sidebar}>
      <div class={styles.header}>
        <i class={`fa-solid fa-users ${styles.headerIcon}`} />
        <span class={styles.headerTitle}>Users</span>
        {data.length > 0 && <span class={styles.headerCount}>{data.length}</span>}
      </div>
      {data.length > 0 && (
        <div class={styles.sortBar}>
          <label class={styles.sortLabel}>
            Sort
            <select
              class={styles.sortSelect}
              value={mode}
              onChange={(e) => pickMode((e.currentTarget as HTMLSelectElement).value as SortMode)}
              aria-label="Sort users"
            >
              {(Object.keys(SORT_LABELS) as SortMode[]).map((m) => (
                <option key={m} value={m}>
                  {SORT_LABELS[m]}
                </option>
              ))}
            </select>
          </label>
          {building != null && (
            <span class={styles.sortLabel} role="status" title="Statistics are being prepared after an upgrade">
              Preparing stats… {building}%
            </span>
          )}
          {building == null && stale && (
            <button
              class={styles.resortBtn}
              onClick={() => setOrder(fresh)}
              title="Apply the sort to the latest numbers"
            >
              <i class="fa-solid fa-arrows-rotate" /> Re-sort
            </button>
          )}
        </div>
      )}

      {data.length === 0 ? (
        <div class={styles.placeholder}>
          <div class={styles.placeholderIcon}>
            <i class="fa-solid fa-user-clock" />
          </div>
          <div class={styles.placeholderTitle}>No users yet</div>
          <div class={styles.placeholderText}>
            Users will appear here as they connect and send requests through the bridge
          </div>
        </div>
      ) : (
        <div class={styles.list}>
          {shown.map((name) => (
            <UserRow key={name} u={byName.get(name)!} mode={mode} />
          ))}
        </div>
      )}
    </aside>
  )
}
