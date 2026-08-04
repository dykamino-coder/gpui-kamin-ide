import { signal } from '@preact/signals'
import { useEffect } from 'preact/hooks'
import { selectTokenId } from '../../signals/ui'
import { formatTokens, formatCost } from '../../utils/formatters'
import { userColor } from '../../utils/user-colors'
import { api } from '../../services/api-client'
import type { UserSummary } from '../../services/api-client'
import styles from './RightSidebar.module.css'

const users = signal<UserSummary[]>([])

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

async function fetchUsers() {
  try {
    users.value = await api.getUsers()
  } catch {}
}

export function RightSidebar() {
  useEffect(() => {
    fetchUsers()
    const id = setInterval(fetchUsers, 15000)
    return () => clearInterval(id)
  }, [])

  const data = users.value

  const handleClick = (name: string) => {
    selectTokenId(name)
  }

  const handleDelete = (e: MouseEvent, name: string) => {
    e.stopPropagation()
    if (!confirm(`Delete all requests from "${name}"?`)) return
    api.deleteByFilter({ userName: name }).then(() => {
      fetchUsers()
    }).catch(() => {})
  }

  return (
    <aside class={styles.sidebar}>
      <div class={styles.header}>
        <i class={`fa-solid fa-users ${styles.headerIcon}`} />
        <span class={styles.headerTitle}>Users</span>
        {data.length > 0 && <span class={styles.headerCount}>{data.length}</span>}
      </div>

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
          {data.map(u => {
            const totalTokens = u.tokens.input + u.tokens.output + u.tokens.cacheRead + u.tokens.cacheCreation
            return (
              <button
                key={u.userName}
                class={styles.userCard}
                onClick={() => handleClick(u.userName)}
                title={u.userName}
              >
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
                    {totalTokens > 0 && (
                      <span class={styles.userMetaItem} title={`Total tokens: ${totalTokens.toLocaleString()}`}>
                        <i class="fa-solid fa-coins" />
                        {formatTokens(totalTokens)}
                      </span>
                    )}
                    {u.contextTokens > 0 && (
                      <span class={styles.userMetaItem} title={`Real conversation context: ${u.contextTokens.toLocaleString()}`} style={{ color: 'var(--accent-purple)' }}>
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
                  {u.lastSeen && (
                    <div class={styles.userTime}>
                      {timeAgo(u.lastSeen)}
                    </div>
                  )}
                </div>
                <button
                  class={styles.deleteBtn}
                  onClick={(e: any) => handleDelete(e, u.userName)}
                  title={`Delete ${u.userName} requests`}
                >
                  <i class="fa-solid fa-trash-can" />
                </button>
              </button>
            )
          })}
        </div>
      )}
    </aside>
  )
}
