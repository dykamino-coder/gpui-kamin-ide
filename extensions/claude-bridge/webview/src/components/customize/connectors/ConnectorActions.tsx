import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { showToast } from '../../../signals/toasts'
import styles from './ConnectorActions.module.css'

interface Props {
  serverId: string
  serverName: string
  enabled: boolean
  autoDiscovered: boolean
  hasOAuth: boolean
  /** Source of this config — determines whether we're allowed to delete the
   *  underlying file entry. Plugin/project sources are read-only for us. */
  sourceType?: string
  onRemoved: () => void
  onEdit: () => void
  onStatusChange: (dot: string, text: string, forServerId: string) => void
}

export function ConnectorActions({ serverId, serverName, enabled, autoDiscovered, hasOAuth, sourceType, onRemoved, onEdit, onStatusChange }: Props): JSX.Element {
  const bridge = useBridge()
  const [testLabel, setTestLabel] = useState('Test')
  const [testDisabled, setTestDisabled] = useState(false)
  const [authDisabled, setAuthDisabled] = useState(false)
  const [authLabel, setAuthLabel] = useState('Connect OAuth')

  async function handleOAuthConnect(): Promise<void> {
    setAuthDisabled(true)
    setAuthLabel('Authorizing…')
    onStatusChange('testing', 'OAuth flow — check your browser', serverId)
    try {
      // Unimplemented host channel resolves null — guard `result?.ok` so a
      // stuck "Authorizing…" button (TypeError on null.ok) can't happen; the
      // finally always re-enables it.
      const result = await bridge.oauthConnectMcpServer(serverId)
      if (result?.ok) {
        showToast({ type: 'success', title: serverName, message: 'OAuth authorization complete' })
        setAuthLabel('Re-auth')
        onStatusChange('connected', 'authorized — reconnecting', serverId)
      } else {
        const msg = result?.error || 'External MCP management is not available yet'
        showToast({ type: 'error', title: serverName, message: msg })
        setAuthLabel('Retry OAuth')
        onStatusChange('error', `oauth error — ${msg}`, serverId)
      }
    } catch (err) {
      showToast({ type: 'error', title: serverName, message: err instanceof Error ? err.message : String(err) })
      setAuthLabel('Retry OAuth')
    } finally {
      setAuthDisabled(false)
    }
  }

  async function handleTest(): Promise<void> {
    setTestLabel('Testing...')
    setTestDisabled(true)
    onStatusChange('testing', 'testing...', serverId)
    try {
      const result = await bridge.testMcpServer(serverId)
      const ok = !!result?.ok
      showToast({
        type: ok ? 'success' : 'error',
        title: serverName,
        message: ok ? `Connected — ${result.tools.length} tools` : (result?.error || 'External MCP management is not available yet'),
      })
      onStatusChange(ok ? 'connected' : 'error', ok ? `connected — ${result.tools.length} tools` : `error — ${result?.error || 'failed'}`, serverId)
      setTestLabel(ok ? 'OK' : 'Fail')
    } catch (err) {
      showToast({ type: 'error', title: serverName, message: err instanceof Error ? err.message : String(err) })
      setTestLabel('Fail')
    } finally {
      setTestDisabled(false)
      setTimeout(() => setTestLabel('Test'), 2000)
    }
  }

  async function handleRemove(): Promise<void> {
    try { await bridge.removeMcpServer(serverId) } catch { /* honest no-op if unimplemented */ }
    onRemoved()
  }

  return (
    <div class={styles.actions}>
      <label class={styles.toggle} title={enabled ? 'Disable' : 'Enable'}>
        <input
          type="checkbox"
          class={styles.toggleInput}
          checked={enabled}
          onChange={(e) => bridge.toggleMcpServer(serverId, (e.target as HTMLInputElement).checked)}
        />
        <span class={styles.toggleSlider} />
      </label>
      <button onClick={handleTest} disabled={testDisabled}>{testLabel}</button>
      {sourceType !== 'plugin' && sourceType !== 'project' && (
        <button onClick={onEdit} data-tooltip="Edit this connector's configuration">Edit</button>
      )}
      {hasOAuth && (
        <button onClick={handleOAuthConnect} disabled={authDisabled} data-tooltip="Run OAuth authorization (Dynamic Client Registration where supported)">
          {authLabel}
        </button>
      )}
      {(!autoDiscovered || sourceType === 'claude-config') && (
        <button class={styles.danger} onClick={handleRemove}>Remove</button>
      )}
    </div>
  )
}
