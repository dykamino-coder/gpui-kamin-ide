import { signal } from '@preact/signals'
import { useEffect } from 'preact/hooks'
import { Modal } from '../shared'
import styles from './ApiTokens.module.css'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ApiToken {
  id: string
  name: string
  token: string
  createdAt: string
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const tokens = signal<ApiToken[]>([])
const loading = signal(false)
const showModal = signal(false)
const newName = signal('')
const creating = signal(false)
const copiedId = signal<string | null>(null)

// ---------------------------------------------------------------------------
// API helpers
// ---------------------------------------------------------------------------

function authHeaders(): Record<string, string> {
  const token = localStorage.getItem('dashboard_session_token')
  return token ? { 'Authorization': `Bearer ${token}` } : {}
}

async function fetchTokens() {
  loading.value = true
  try {
    const res = await fetch('/api/dashboard/tokens', { headers: authHeaders() })
    if (res.ok) tokens.value = await res.json()
  } finally {
    loading.value = false
  }
}

async function handleCreate() {
  if (!newName.value.trim()) return
  creating.value = true
  try {
    const res = await fetch('/api/dashboard/tokens', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...authHeaders() },
      body: JSON.stringify({ name: newName.value.trim() }),
    })
    if (res.ok) {
      await fetchTokens()
      showModal.value = false
      newName.value = ''
    }
  } finally {
    creating.value = false
  }
}

async function handleDelete(id: string) {
  await fetch(`/api/dashboard/tokens/${id}`, { method: 'DELETE', headers: authHeaders() })
  await fetchTokens()
}

function copyToClipboard(text: string, id: string) {
  // execCommand fallback works on HTTP; clipboard API requires secure context
  const fallback = () => {
    const ta = document.createElement('textarea')
    ta.value = text
    ta.style.cssText = 'position:fixed;opacity:0;left:-9999px'
    document.body.appendChild(ta)
    ta.focus()
    ta.select()
    try { document.execCommand('copy') } catch {}
    document.body.removeChild(ta)
  }
  if (navigator.clipboard?.writeText) {
    navigator.clipboard.writeText(text).catch(fallback)
  } else {
    fallback()
  }
  copiedId.value = id
  setTimeout(() => { if (copiedId.value === id) copiedId.value = null }, 1500)
}

function maskToken(token: string): string {
  if (token.length <= 8) return token
  return token.slice(0, 4) + '•'.repeat(Math.min(token.length - 8, 24)) + token.slice(-4)
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-GB', { day: '2-digit', month: '2-digit', year: 'numeric' })
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ApiTokens() {
  useEffect(() => { fetchTokens() }, [])

  const openCreate = () => {
    newName.value = ''
    showModal.value = true
  }

  return (
    <div class={styles.container}>
      {/* Header */}
      <div class={styles.header}>
        <i class={`fa-solid fa-key ${styles.headerIcon}`} />
        API Tokens
        <span class={styles.subtitle}>{tokens.value.length} tokens</span>
        <button class={styles.createBtn} onClick={openCreate}>
          <i class="fa-solid fa-plus" />
          Create Token
        </button>
      </div>

      {/* Token table */}
      {tokens.value.length === 0 ? (
        <div class={styles.empty}>
          <i class="fa-solid fa-key" />
          <div>No tokens yet. Create one to start using the API.</div>
        </div>
      ) : (
        <div class={styles.tableWrap}>
          <table class={styles.table}>
            <thead>
              <tr>
                <th>Name</th>
                <th>Token</th>
                <th>Created</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {tokens.value.map((t) => (
                <tr key={t.id}>
                  <td class={styles.cellName}>{t.name}</td>
                  <td class={styles.cellToken}>
                    <code class={styles.tokenCode}>{maskToken(t.token)}</code>
                    <button
                      class={styles.copyBtn}
                      onClick={() => copyToClipboard(t.token, t.id)}
                      title="Copy token"
                    >
                      <i class={copiedId.value === t.id ? 'fa-solid fa-check' : 'fa-solid fa-copy'} />
                    </button>
                  </td>
                  <td class={styles.cellDate}>{formatDate(t.createdAt)}</td>
                  <td class={styles.cellActions}>
                    <button class={styles.deleteBtn} onClick={() => handleDelete(t.id)} title="Delete token">
                      <i class="fa-solid fa-trash" />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Usage instructions */}
      <div class={styles.guide}>
        <div class={styles.guideHeader}>
          <i class="fa-solid fa-plug" />
          How to Connect
        </div>
        <div class={styles.guideContent}>
          <ol style={{ margin: 0, paddingLeft: '20px', color: 'var(--text-secondary)', fontSize: '13px', lineHeight: '1.8' }}>
            <li>Create a token above</li>
            <li>Download and install the KaminIDE client</li>
            <li>Enter the server URL and your token in the client settings</li>
            <li>Click Connect — you're ready to use Claude</li>
          </ol>
        </div>
      </div>

      {/* Create modal */}
      <Modal isOpen={showModal.value} onClose={() => { showModal.value = false }} title="Create API Token" size="sm"
        footer={
          <>
            <button class={styles.cancelBtn} onClick={() => { showModal.value = false }}>Cancel</button>
            <button class={styles.submitBtn} onClick={handleCreate} disabled={creating.value || !newName.value.trim()}>
              {creating.value ? 'Creating...' : 'Create'}
            </button>
          </>
        }
      >
        <div class={styles.formGroup}>
          <label class={styles.label}>Name</label>
          <input
            class={styles.input}
            type="text"
            placeholder="e.g. My Laptop, John, Work PC"
            value={newName.value}
            onInput={(e) => { newName.value = (e.target as HTMLInputElement).value }}
            onKeyDown={(e) => { if (e.key === 'Enter') handleCreate() }}
            autoFocus
          />
        </div>
      </Modal>
    </div>
  )
}
