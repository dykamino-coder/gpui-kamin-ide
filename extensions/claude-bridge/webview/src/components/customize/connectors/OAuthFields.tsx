// HTTP-only OAuth 2.0 (PKCE) field group extracted from AddServerForm.tsx
// (Sprint 5 / Stage E1). When enabled the form save handler triggers
// `oauthConnectMcpServer` immediately after persisting the config.

import type { JSX } from 'preact'
import { Field } from '../../ui/Field'
import { Switch } from '../../ui/Switch'
import styles from './AddServerForm.module.css'

interface Props {
  useOAuth: boolean
  authServerUrl: string
  clientId: string
  clientSecret: string
  scope: string
  onUseOAuthChange: (v: boolean) => void
  onAuthServerUrlChange: (v: string) => void
  onClientIdChange: (v: string) => void
  onClientSecretChange: (v: string) => void
  onScopeChange: (v: string) => void
}

export function OAuthFields({
  useOAuth, authServerUrl, clientId, clientSecret, scope,
  onUseOAuthChange, onAuthServerUrlChange, onClientIdChange, onClientSecretChange, onScopeChange,
}: Props): JSX.Element {
  return (
    <div style="margin-top:12px;padding:12px;background:var(--bg-primary);border:1px solid var(--bg-surface);border-radius:var(--radius-sm)">
      <Switch
        checked={useOAuth}
        onChange={onUseOAuthChange}
        label="Use OAuth 2.0 (PKCE)"
      />
      {useOAuth && (
        <div style="margin-top:10px;display:flex;flex-direction:column;gap:8px">
          <Field label="Authorization server URL" required>
            <input
              class={styles.input}
              type="text"
              placeholder="https://example.com/.well-known/oauth-authorization-server"
              value={authServerUrl}
              onInput={(e) => onAuthServerUrlChange((e.target as HTMLInputElement).value)}
            />
          </Field>
          <Field label="Client ID (optional — omit for Dynamic Client Registration)">
            <input
              class={styles.input}
              type="text"
              value={clientId}
              onInput={(e) => onClientIdChange((e.target as HTMLInputElement).value)}
            />
          </Field>
          <Field label="Client Secret (optional, public clients leave empty)">
            <input
              class={styles.input}
              type="password"
              value={clientSecret}
              onInput={(e) => onClientSecretChange((e.target as HTMLInputElement).value)}
            />
          </Field>
          <Field
            label="Scope (optional)"
            hint="After save, your default browser will open for authorization. Tokens are stored encrypted locally and refreshed automatically."
          >
            <input
              class={styles.input}
              type="text"
              placeholder="e.g. read write offline_access"
              value={scope}
              onInput={(e) => onScopeChange((e.target as HTMLInputElement).value)}
            />
          </Field>
        </div>
      )}
    </div>
  )
}
