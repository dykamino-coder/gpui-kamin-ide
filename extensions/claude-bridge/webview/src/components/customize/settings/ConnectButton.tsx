import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { setConfigAndHandleTokenChange } from '../../../lib/tabs-persist'
import { serverFetch } from '../../../lib/server-fetch'
import styles from './ConnectButton.module.css'

interface Props {
  serverUrl: string
  token: string
  onTokenChange: (token: string) => void
  whisperUrl: string
  whisperModel: string
  whisperToken: string
}

interface TestResult {
  ok: boolean
  tokenName?: string
  createdAt?: string
  error?: string
}

export function ConnectButton({ serverUrl, token, onTokenChange, whisperUrl, whisperModel, whisperToken }: Props): JSX.Element {
  const bridge = useBridge()
  const [testResult, setTestResult] = useState<TestResult | null>(null)
  const [testing, setTesting] = useState(false)
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [newTokenName, setNewTokenName] = useState('')
  const [creating, setCreating] = useState(false)

  const hasToken = !!token.trim()
  const httpUrl = serverUrl.replace(/^ws/, 'http').replace(/\/ws\/session$/, '')

  function saveConfig(tok?: string): void {
    const effective = tok || token
    void setConfigAndHandleTokenChange(bridge, { serverUrl, token: effective, whisperUrl: whisperUrl || undefined, whisperModel: whisperModel || undefined, whisperToken: whisperToken || undefined })
  }

  async function handleTestConnect(): Promise<void> {
    if (!serverUrl) { setTestResult({ ok: false, error: 'Server URL is required' }); return }
    if (!token) { setTestResult({ ok: false, error: 'Token is required' }); return }
    setTesting(true); setTestResult(null)
    try {
      // Resolve the token we hold → its {name, createdAt} without listing every
      // user's raw token (the server no longer returns raw values to non-admins).
      const res = await serverFetch(bridge, httpUrl, '/api/dashboard/tokens/resolve', {
        method: 'POST', body: JSON.stringify({ token }),
      })
      if (res.ok) {
        const match = res.data as { name: string; createdAt: string }
        setTestResult({ ok: true, tokenName: match.name, createdAt: match.createdAt })
        saveConfig()
      } else if (res.status === 404) {
        setTestResult({ ok: false, error: 'Token not found on server' })
      } else {
        throw new Error(res.error || `HTTP ${res.status}`)
      }
    } catch (err) {
      setTestResult({ ok: false, error: err instanceof Error ? err.message : String(err) })
    } finally { setTesting(false) }
  }

  async function handleCreateToken(): Promise<void> {
    if (!newTokenName.trim()) return
    setCreating(true)
    try {
      // Check for duplicate name
      const listRes = await serverFetch(bridge, httpUrl, '/api/dashboard/tokens')
      if (listRes.ok) {
        const existing = listRes.data as Array<{ name: string }>
        if (existing.some(t => t.name === newTokenName.trim())) {
          setTestResult({ ok: false, error: `Token "${newTokenName.trim()}" already exists` })
          setCreating(false); return
        }
      }
      const res = await serverFetch(bridge, httpUrl, '/api/dashboard/tokens', {
        method: 'POST', body: JSON.stringify({ name: newTokenName.trim() }),
      })
      if (!res.ok) throw new Error(res.error || `HTTP ${res.status}`)
      const data = res.data as { token: string; name: string; createdAt: string }
      onTokenChange(data.token)
      setShowCreateModal(false); setNewTokenName('')
      setTestResult({ ok: true, tokenName: data.name, createdAt: data.createdAt })
      saveConfig(data.token)
    } catch (err) {
      setTestResult({ ok: false, error: err instanceof Error ? err.message : String(err) })
    } finally { setCreating(false) }
  }

  async function handleNewSession(): Promise<void> {
    if (!serverUrl || !token) return
    try {
      saveConfig()
      const cwd = await bridge.getCwd()
      await bridge.createTab({ serverUrl, token, cwd: cwd || undefined })
      // In the Electron app this navigated to the sessions view. Here each
      // customize section is its own always-alive page — nulling
      // activeCustomizePanel just blanked THIS page forever (the URL effect
      // runs once). The tab is created; the user opens the chat panel.
      setTestResult({ ok: true, tokenName: 'Session created — open the Claude chat panel' })
    } catch (err) {
      setTestResult({ ok: false, error: err instanceof Error ? err.message : String(err) })
    }
  }

  return (
    <>
      <div style="display:flex;gap:8px;margin-top:4px">
        {hasToken ? (
          <button class={styles.btn} onClick={handleTestConnect} disabled={testing} style="flex:1">
            {testing ? <><i class="fas fa-spinner fa-spin" style="margin-right:6px" />Testing...</> : <><i class="fas fa-plug" style="margin-right:6px" />Test Connect</>}
          </button>
        ) : (
          <button
            class={styles.btn}
            onClick={async () => {
              setShowCreateModal(true)
              // Seed the token name with the machine's "<domain>\<user>" (same as
              // the first-run gate does) so the user doesn't invent a name. Only
              // when empty — don't clobber a name they already typed.
              if (!newTokenName) {
                try { const id = await bridge.getMachineId(); if (typeof id === 'string' && id) setNewTokenName(id) } catch { /* leave empty */ }
              }
            }}
            style="flex:1;background:var(--accent-green)"
          >
            <i class="fas fa-key" style="margin-right:6px" />Create Token
          </button>
        )}
        {hasToken && (
          <button class={styles.btn} onClick={handleNewSession} style="flex:1;background:var(--accent-green)">
            <i class="fas fa-circle-plus" style="margin-right:6px" />New Default Session
          </button>
        )}
      </div>

      {testResult && (
        <div style={`margin-top:8px;padding:8px 12px;border-radius:var(--radius-sm);font-size:12px;border:1px solid ${testResult.ok ? 'var(--accent-green)' : 'var(--accent-red)'};background:${testResult.ok ? 'var(--bg-tint-green)' : 'var(--bg-tint-orange)'}`}>
          {testResult.ok ? (
            <div style="display:flex;align-items:center;gap:8px">
              <i class="fas fa-circle-check" style="color:var(--accent-green)" />
              <div>
                <div style="color:var(--accent-green);font-weight:600">Connected</div>
                <div style="color:var(--text-muted);font-size:11px;margin-top:2px">
                  Token: <span style="color:var(--text-primary)">{testResult.tokenName}</span>
                  {testResult.createdAt && <> · Created: <span style="color:var(--text-primary)">{new Date(testResult.createdAt).toLocaleDateString()}</span></>}
                </div>
              </div>
            </div>
          ) : (
            <div style="display:flex;align-items:center;gap:8px">
              <i class="fas fa-circle-xmark" style="color:var(--accent-red)" />
              <span style="color:var(--accent-red)">{testResult.error}</span>
            </div>
          )}
        </div>
      )}

      {showCreateModal && (
        <div style="position:fixed;inset:0;background:var(--overlay-deep);z-index:10000;display:flex;align-items:center;justify-content:center" onClick={(e) => { if (e.target === e.currentTarget) setShowCreateModal(false) }}>
          <div style="background:var(--bg-primary);border:1px solid var(--bg-surface);border-radius:var(--radius-md);padding:24px;width:380px;box-shadow:var(--shadow-modal)">
            <div style="font-size:16px;font-weight:600;color:var(--text-primary);margin-bottom:16px;display:flex;align-items:center;gap:10px">
              <i class="fas fa-key" style="color:var(--accent-green)" /> Create API Token
            </div>
            <label style="font-size:12px;color:var(--text-muted);display:block;margin-bottom:4px">Token Name</label>
            <input
              type="text"
              value={newTokenName}
              onInput={(e) => setNewTokenName((e.target as HTMLInputElement).value)}
              onKeyDown={(e) => { if (e.key === 'Enter') handleCreateToken() }}
              placeholder="e.g. my-kaminide-client"
              autoFocus
              style="width:100%;padding:10px 12px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);color:var(--text-primary);font-size: var(--fs-md);outline:none;box-sizing:border-box"
            />
            <div style="display:flex;gap:8px;margin-top:16px;justify-content:flex-end">
              <button onClick={() => setShowCreateModal(false)} style="padding:8px 16px;background:transparent;border:1px solid var(--bg-overlay);color:var(--text-muted);border-radius:var(--radius-sm);cursor:pointer;font-size:12px">Cancel</button>
              <button onClick={handleCreateToken} disabled={creating || !newTokenName.trim()} style="padding:8px 16px;background:var(--accent-green);color:var(--bg-primary);border:none;border-radius:var(--radius-sm);cursor:pointer;font-size:12px;font-weight:600">
                {creating ? 'Creating...' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  )
}
