import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { Button } from '../../ui/Button'
import styles from './MarketplaceCloneForm.module.css'

interface Props {
  onDone: (name: string) => void
  onCancel: () => void
}

export function MarketplaceCloneForm({ onDone, onCancel }: Props): JSX.Element {
  const bridge = useBridge()
  const [url, setUrl] = useState('')
  const [token, setToken] = useState('')
  const [username, setUsername] = useState('oauth2')
  const [showAuth, setShowAuth] = useState(false)
  const [label, setLabel] = useState('Add')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Derive a marketplace name that's stable across the three input shapes
  // Claude Code's known_marketplaces.json supports: git URL, owner/repo
  // shortform, or absolute local path. Back end decides the source type,
  // we only need the bare directory-ish name here for the registry key.
  function deriveName(raw: string): string {
    const trimmed = raw.trim()
    if (!trimmed) return 'marketplace'
    const cleaned = trimmed.replace(/[\/\\]+$/, '').replace(/\.git$/, '')
    const seg = cleaned.split(/[\/\\]/).pop() || 'marketplace'
    // Sanitize to match VALID_NAME on the backend (/^[A-Za-z0-9._-]{1,64}$/)
    return seg.replace(/[^A-Za-z0-9._-]/g, '-').slice(0, 64) || 'marketplace'
  }

  async function handleClone(): Promise<void> {
    const raw = url.trim()
    if (!raw || busy) return
    const name = deriveName(raw)
    setBusy(true)
    setLabel('Adding…')
    setError(null)
    try {
      const auth = token.trim() ? { username: username.trim() || 'oauth2', token: token.trim() } : null
      await bridge.addMarketplace(name, raw, auth)
      setUrl('')
      setToken('')
      setLabel('Add')
      setBusy(false)
      onDone(name)
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      setLabel('Retry')
      setError(msg)
      setBusy(false)
    }
  }

  return (
    <div class={styles.form}>
      <div class={styles.row}>
        <input
          class={styles.input}
          type="text"
          placeholder="owner/repo · https://github.com/user/repo.git · /absolute/local/path"
          value={url}
          disabled={busy}
          onInput={(e) => setUrl((e.target as HTMLInputElement).value)}
          onKeyDown={(e) => { if (e.key === 'Enter') handleClone() }}
        />
        <Button variant="primary" onClick={handleClone} disabled={busy}>{label}</Button>
        <Button variant="secondary" onClick={onCancel} disabled={busy}>Cancel</Button>
      </div>
      {!showAuth && (
        <button class={styles.toggleAuth} onClick={() => setShowAuth(true)}>
          + Add personal access token (for private repos)
        </button>
      )}
      {showAuth && (
        <>
          <div class={styles.row}>
            <input
              class={`${styles.input} ${styles.inputNarrow}`}
              type="text"
              placeholder="username (oauth2)"
              value={username}
              disabled={busy}
              onInput={(e) => setUsername((e.target as HTMLInputElement).value)}
            />
            <input
              class={styles.input}
              type="password"
              placeholder="personal access token (glpat-… / ghp_…)"
              value={token}
              disabled={busy}
              onInput={(e) => setToken((e.target as HTMLInputElement).value)}
              onKeyDown={(e) => { if (e.key === 'Enter') handleClone() }}
            />
          </div>
          <div class={styles.hint}>
            Token is baked into the URL saved in <code>~/.claude/plugins/known_marketplaces.json</code> so the Claude Code CLI can clone/pull the same repo. For GitLab use <code>oauth2</code> as username with a Personal Access Token (scope: <code>read_repository</code>).
          </div>
        </>
      )}
      {error && (
        <div class={styles.error}>{error}</div>
      )}
    </div>
  )
}
