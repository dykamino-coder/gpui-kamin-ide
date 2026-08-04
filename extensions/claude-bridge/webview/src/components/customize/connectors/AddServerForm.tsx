import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import type { ExternalMcpServerInfo, ExternalMcpServerConfig } from '../../../../shared/types'
import { ServerTypeSelect } from './ServerTypeSelect'
import { HttpFields } from './HttpFields'
import { StdioFields } from './StdioFields'
import { OAuthFields } from './OAuthFields'
import { PresetButtonRow } from './PresetButtonRow'
import { PRESETS } from './add-server-presets'
import type { PresetDefinition, PresetContext, ServerType } from './add-server-presets'
import { Field } from '../../ui/Field'
import { Button } from '../../ui/Button'
import styles from './AddServerForm.module.css'

interface Props {
  onCancel: () => void
  onSaved: () => void
  /** When provided, the form edits this existing server instead of adding a
   *  new one (fields pre-filled, Save calls updateMcpServer). */
  initial?: ExternalMcpServerInfo
}

export function AddServerForm({ onCancel, onSaved, initial }: Props): JSX.Element {
  const bridge = useBridge()
  const isEdit = !!initial
  const [name, setName] = useState(initial?.name ?? '')
  const [type, setType] = useState<ServerType>((initial?.type as ServerType) ?? 'stdio')
  const [url, setUrl] = useState(initial?.url ?? '')
  const [headers, setHeaders] = useState(initial?.headers ? JSON.stringify(initial.headers, null, 2) : '')
  const [command, setCommand] = useState(initial?.command ?? '')
  const [args, setArgs] = useState(initial?.args ? initial.args.join(', ') : '')
  const [env, setEnv] = useState(initial?.env ? JSON.stringify(initial.env, null, 2) : '')

  const [useOAuth, setUseOAuth] = useState(!!initial?.oauth?.authServerUrl)
  const [authServerUrl, setAuthServerUrl] = useState(initial?.oauth?.authServerUrl ?? '')
  const [clientId, setClientId] = useState(initial?.oauth?.clientId ?? '')
  const [clientSecret, setClientSecret] = useState(initial?.oauth?.clientSecret ?? '')
  const [scope, setScope] = useState(initial?.oauth?.scope ?? '')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Existing server names — used to hide presets that would collide with an
  // already-configured connector. Refresh on the server-list channel so the
  // preset row updates live while the form is open.
  const [existingNames, setExistingNames] = useState<Set<string>>(new Set())
  useEffect(() => {
    bridge.getMcpServers()
      .then(list => setExistingNames(new Set(list.map(s => s.name.toLowerCase()))))
      .catch(() => {})
    return bridge.onMcpServersChanged((list) => {
      setExistingNames(new Set(list.map(s => s.name.toLowerCase())))
    })
  }, [bridge])

  const availablePresets = PRESETS.filter(p => !existingNames.has(p.key))

  const presetCtx: PresetContext = {
    setName, setType, setUrl, setHeaders, setCommand, setArgs, setEnv,
    setUseOAuth, setAuthServerUrl, setClientId, setClientSecret, setScope,
  }

  function applyPreset(preset: PresetDefinition): void {
    setError(null)
    preset.apply(presetCtx)
  }

  async function handleSave(): Promise<void> {
    setError(null)
    if (!name) { setError('Name is required'); return }
    // Block a name that collides with a DIFFERENT connector — ~/.claude.json is
    // keyed by name, so a duplicate would silently overwrite the other on disk.
    // (existingNames is lowercased; exclude our own name when editing.)
    const nameLc = name.trim().toLowerCase()
    if (nameLc !== (initial?.name.toLowerCase() ?? '') && existingNames.has(nameLc)) {
      setError(`A connector named "${name.trim()}" already exists`); return
    }
    const isHttp = type === 'http' || type === 'sse' || type === 'ws'

    // Set EVERY editable field (undefined to clear). On edit the host merges
    // this over the existing config, so an unset key would leave the old value
    // behind — explicitly nulling the other transport's fields makes switching
    // type / turning OAuth off actually take effect.
    // `any` because the same object feeds two different channel shapes —
    // addMcpServer wants Omit<Config,'id'> (all required), updateMcpServer wants
    // Partial — and neither alone types the incremental builder below cleanly.
    const config: any = {
      name, type, enabled: initial?.enabled ?? true,
      url: undefined, headers: undefined, command: undefined, args: undefined, env: undefined, oauth: undefined,
    }
    if (isHttp) {
      if (!url) { setError('URL is required'); return }
      config.url = url
      if (headers) {
        try { config.headers = JSON.parse(headers) } catch { setError('Headers JSON is invalid'); return }
      }
      if (useOAuth) {
        if (!authServerUrl) { setError('Authorization server URL is required'); return }
        // clientId is optional here — when omitted, the oauth-connect handler
        // runs RFC 7591 Dynamic Client Registration against the AS (required
        // for providers like Figma that don't publish a fixed one).
        config.oauth = {
          authServerUrl,
          clientId: clientId || '',
          ...(clientSecret ? { clientSecret } : {}),
          ...(scope ? { scope } : {}),
          // Preserve a deliberately-pinned callback port across edits (it isn't
          // exposed as a field) so a registered redirect URI keeps working.
          ...(initial?.oauth?.callbackPort ? { callbackPort: initial.oauth.callbackPort } : {}),
        }
      }
    } else {
      if (!command) { setError('Command is required'); return }
      config.command = command
      if (args) config.args = args.split(',').map((s: string) => s.trim()).filter(Boolean)
      if (env) {
        try { config.env = JSON.parse(env) } catch { setError('Env JSON is invalid'); return }
      }
    }

    setSaving(true)
    try {
      const saved = isEdit
        ? await bridge.updateMcpServer(initial!.id, config)
        : await bridge.addMcpServer(config)
      // Unimplemented host channel resolves null — don't report a fake success
      // (clear the form + onSaved) when nothing was actually saved.
      if (!saved) {
        setError('External MCP management is not available in KaminIDE yet')
        return
      }
      // Only kick off the OAuth browser flow when ADDING a server. On edit the
      // user re-runs it explicitly via the "Connect OAuth" button in the detail
      // view (re-authorizing on every config tweak would be intrusive).
      if (!isEdit && useOAuth && saved.id) {
        const result = await bridge.oauthConnectMcpServer(saved.id)
        if (!result?.ok) {
          setError(result?.error || 'OAuth authorization failed')
          return
        }
      }
      setName(''); setUrl(''); setHeaders(''); setCommand(''); setArgs(''); setEnv('')
      setUseOAuth(false); setAuthServerUrl(''); setClientId(''); setClientSecret(''); setScope('')
      onSaved()
    } catch (err: any) {
      setError(err?.message || String(err))
    } finally {
      setSaving(false)
    }
  }

  const isHttp = type === 'http' || type === 'sse' || type === 'ws'

  return (
    <div class={styles.form}>
      <div class={styles.header}>
        <h2>{isEdit ? 'Edit Connector' : 'Add Connector'}</h2>
      </div>
      <div class={styles.section}>
        {!isEdit && <PresetButtonRow presets={availablePresets} onApply={applyPreset} />}
        <Field label="Name" required>
          <input
            class={styles.input}
            type="text"
            placeholder="e.g. GitHub MCP"
            value={name}
            onInput={(e) => setName((e.target as HTMLInputElement).value)}
          />
        </Field>
        <ServerTypeSelect value={type} onChange={setType} />
        {isHttp
          ? <HttpFields url={url} headers={headers} onUrlChange={setUrl} onHeadersChange={setHeaders} />
          : <StdioFields command={command} args={args} env={env} onCommandChange={setCommand} onArgsChange={setArgs} onEnvChange={setEnv} />
        }
        {isHttp && (
          <OAuthFields
            useOAuth={useOAuth}
            authServerUrl={authServerUrl}
            clientId={clientId}
            clientSecret={clientSecret}
            scope={scope}
            onUseOAuthChange={setUseOAuth}
            onAuthServerUrlChange={setAuthServerUrl}
            onClientIdChange={setClientId}
            onClientSecretChange={setClientSecret}
            onScopeChange={setScope}
          />
        )}
        {error && (
          <div style="margin-top:10px;padding:8px 10px;background:color-mix(in srgb, var(--accent-red) 13%, transparent);border:1px solid var(--accent-red);border-radius:var(--radius-sm);color:var(--accent-red);font-size:12px">
            {error}
          </div>
        )}
        <div class={styles.buttons}>
          <Button variant="primary" class={styles.grow} onClick={handleSave} disabled={saving}>
            {saving ? (isEdit ? 'Saving…' : 'Connecting…') : (isEdit ? 'Save changes' : 'Save')}
          </Button>
          <Button variant="secondary" class={styles.grow} onClick={onCancel} disabled={saving}>Cancel</Button>
        </div>
      </div>
    </div>
  )
}
