import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { showToast } from '../../../signals/toasts'
import styles from './SetTokenModal.module.css'

interface Props {
  marketplaceName: string
  onClose: () => void
  onDone?: () => void
}

export function SetTokenModal({ marketplaceName, onClose, onDone }: Props): JSX.Element {
  const bridge = useBridge()
  const [username, setUsername] = useState('oauth2')
  const [token, setToken] = useState('')
  const [saving, setSaving] = useState(false)

  async function handleSave(next: 'set' | 'clear'): Promise<void> {
    setSaving(true)
    try {
      const auth = next === 'clear' ? null : { username: username.trim() || 'oauth2', token: token.trim() }
      // Empty token on 'set' is treated the same as 'clear' by the backend.
      const result = await bridge.setMarketplaceAuth(marketplaceName, auth)
      if (!result.ok) {
        showToast({ type: 'error', title: marketplaceName, message: result.error || 'Could not save auth' })
        setSaving(false)
        return
      }
      if (result.warning) {
        showToast({ type: 'warning' as any, title: marketplaceName, message: result.warning })
      } else {
        showToast({
          type: 'success',
          title: marketplaceName,
          message: next === 'clear' ? 'Credentials cleared' : 'Token updated',
        })
      }
      onDone?.()
      onClose()
    } catch (err: any) {
      showToast({ type: 'error', title: marketplaceName, message: err?.message || String(err) })
      setSaving(false)
    }
  }

  return (
    <div class={styles.backdrop} onClick={onClose}>
      <div class={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <div class={styles.header}>
          <h3>{marketplaceName} — access token</h3>
          <button class={styles.closeBtn} onClick={onClose}><i class="fas fa-times" /></button>
        </div>
        <div class={styles.body}>
          <div class={styles.field}>
            <label class={styles.label}>Username</label>
            <input
              class={styles.input}
              type="text"
              value={username}
              disabled={saving}
              onInput={(e) => setUsername((e.target as HTMLInputElement).value)}
              placeholder="oauth2"
            />
          </div>
          <div class={styles.field}>
            <label class={styles.label}>Personal access token</label>
            <input
              class={styles.input}
              type="password"
              value={token}
              disabled={saving}
              onInput={(e) => setToken((e.target as HTMLInputElement).value)}
              onKeyDown={(e) => { if (e.key === 'Enter') handleSave('set') }}
              placeholder="glpat-… / ghp_…"
            />
          </div>
          <div class={styles.hint}>
            Rewrites <code>source.url</code> in <code>~/.claude/plugins/known_marketplaces.json</code> and updates the local clone's <code>origin</code> remote. For GitLab use <code>oauth2</code> as username with a PAT (scope: <code>read_repository</code>).
          </div>
        </div>
        <div class={styles.footer}>
          <button class={styles.dangerBtn} onClick={() => handleSave('clear')} disabled={saving} title="Remove any embedded credentials from this marketplace's URL">
            Clear credentials
          </button>
          <div class={styles.spacer} />
          <button class={styles.cancelBtn} onClick={onClose} disabled={saving}>Cancel</button>
          <button class={styles.saveBtn} onClick={() => handleSave('set')} disabled={saving || !token.trim()}>
            {saving ? 'Saving…' : 'Save token'}
          </button>
        </div>
      </div>
    </div>
  )
}
