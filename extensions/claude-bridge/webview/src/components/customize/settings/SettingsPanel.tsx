import type { JSX } from 'preact'
import { useState, useEffect, useRef } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { showToast } from '../../../signals/toasts'
import { SettingsSection } from './SettingsSection'
import { SettingsInput } from './SettingsInput'
import { ConnectButton } from './ConnectButton'
import { ExportTranscriptsButton } from './ExportTranscriptsButton'
import { WhisperSettings } from './WhisperSettings'
import { Switch } from '../../ui/Switch'
import { hasToken } from '../../../signals/ui'
import { setConfigAndHandleTokenChange } from '../../../lib/tabs-persist'
import { serverFetch } from '../../../lib/server-fetch'
import styles from './SettingsPanel.module.css'

export function SettingsPanel(): JSX.Element {
  const bridge = useBridge()
  const [serverUrl, setServerUrl] = useState('ws://localhost:3456')
  const [token, setToken] = useState('')
  const [whisperUrl, setWhisperUrl] = useState('')
  const [whisperModel, setWhisperModel] = useState('')
  const [whisperToken, setWhisperToken] = useState('')
  const [tokenName, setTokenName] = useState<string | null>(null)
  const [tokenIdResolved, setTokenIdResolved] = useState<string | null>(null)
  const [streamingEnabled, setStreamingEnabled] = useState(true)
  const [streamingShowSide, setStreamingShowSide] = useState(false)
  const [streamingShowThinking, setStreamingShowThinking] = useState(true)
  const [streamingLoading, setStreamingLoading] = useState(false)
  const [streamingSaveBusy, setStreamingSaveBusy] = useState(false)
  const [basePrompt, setBasePrompt] = useState('')
  const [basePromptSaved, setBasePromptSaved] = useState(false)

  // Don't auto-save while the initial getConfig is still populating the form.
  const hydrated = useRef(false)

  useEffect(() => {
    bridge.getConfig().then((config) => {
      setServerUrl(config.serverUrl || 'ws://localhost:3456')
      setToken(config.token || '')
      setWhisperUrl(config.whisperUrl || '')
      setWhisperModel(config.whisperModel || '')
      setWhisperToken(config.whisperToken || '')
      hydrated.current = true
    }).catch(() => { hydrated.current = true })
    // Standing instructions live outside `config` (keyed by token, so a token
    // switch can't carry one workspace's prompt into another).
    bridge.getBasePrompt().then(setBasePrompt).catch(() => { /* none yet */ })
  }, [bridge])

  // Persist field edits as they happen (debounced). The panel has no Save
  // button, and the old unmount-only cleanup NEVER ran here: each customize
  // section is an always-alive iframe — it doesn't unmount on section switch,
  // and iframe teardown doesn't run Preact cleanups — so token/serverUrl
  // edits were silently lost.
  useEffect(() => {
    if (!hydrated.current) return
    const timer = setTimeout(() => {
      const tok = token.trim()
      void setConfigAndHandleTokenChange(bridge, {
        serverUrl,
        token: tok,
        whisperUrl: whisperUrl || undefined,
        whisperModel: whisperModel || undefined,
        whisperToken: whisperToken || undefined,
      })
      if (!tok) hasToken.value = false
    }, 600)
    return () => clearTimeout(timer)
  }, [token, serverUrl, whisperUrl, whisperModel, whisperToken])

  // Resolve token → name by querying the server's dashboard API. Cheap
  // lookup (one HTTP GET) and gives the user confirmation that the
  // currently-saved token actually corresponds to a known account without
  // having to click "Test Connect".
  // RETRY, don't fire-and-forget. This runs on mount, so it can race the
  // ext-host that actually performs the fetch (serverFetch → `bridge:server-fetch`
  // invoke). A lost/early invoke NEVER settles — `inv()` has no timeout — so the
  // old single-shot `.catch(() => {})` left `tokenIdResolved` null FOREVER (the
  // deps below never change again), which silently hides the whole streaming
  // section behind its "connect with a valid token" placeholder. Meanwhile "Test
  // Connect" resolves the SAME endpoint fine because it runs on a click, long
  // after everything is up — the exact "badge says Connected but the checkboxes
  // are gone" report. So: bound each attempt with a timeout and retry a few times.
  useEffect(() => {
    setTokenName(null)
    if (!token || !serverUrl) return
    const httpUrl = serverUrl.replace(/^ws/, 'http').replace(/\/ws\/session$/, '')
    let cancelled = false
    const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms))
    async function resolveToken(): Promise<void> {
      for (let attempt = 1; attempt <= 5 && !cancelled; attempt++) {
        const res = await Promise.race([
          serverFetch(bridge, httpUrl, '/api/dashboard/tokens/resolve', {
            method: 'POST', body: JSON.stringify({ token }),
          }).catch(() => null),
          sleep(4000).then(() => null), // the invoke may never settle — bound it
        ])
        if (cancelled) return
        if (res?.ok) {
          const match = res.data as { id?: string; name?: string }
          setTokenName(match.name ?? null)
          setTokenIdResolved(match.id ?? null)
          return
        }
        // 404 = this token genuinely isn't on the server; retrying can't help.
        if (res && res.status === 404) return
        await sleep(600 * attempt) // transient (host not up yet / offline) — back off
      }
    }
    void resolveToken()
    return () => { cancelled = true }
  }, [token, serverUrl])

  // Load streaming settings for the resolved tokenId.
  useEffect(() => {
    if (!tokenIdResolved || !serverUrl) return
    const httpUrl = serverUrl.replace(/^ws/, 'http').replace(/\/ws\/session$/, '')
    let cancelled = false
    setStreamingLoading(true)
    serverFetch(bridge, httpUrl, `/api/dashboard/tokens/${encodeURIComponent(tokenIdResolved)}/streaming-settings`)
      .then(r => r.ok ? r.data : null)
      .then((s: any) => {
        if (cancelled || !s) return
        if (typeof s.enabled === 'boolean') setStreamingEnabled(s.enabled)
        if (typeof s.showSideQuests === 'boolean') setStreamingShowSide(s.showSideQuests)
        if (typeof s.showThinkingStream === 'boolean') setStreamingShowThinking(s.showThinkingStream)
      })
      .catch(() => {})
      .finally(() => { if (!cancelled) setStreamingLoading(false) })
    return () => { cancelled = true }
  }, [tokenIdResolved, serverUrl])

  // The toggle updates the UI optimistically, then persists. If the write
  // fails (offline / server error) we ROLL BACK the optimistic change and tell
  // the user — otherwise the switch silently disagrees with what the backend
  // actually stored (the desync the analysis flagged). `revert` restores the
  // previous value of whichever toggle was flipped.
  async function persistStreaming(
    next: { enabled?: boolean; showSideQuests?: boolean; showThinkingStream?: boolean },
    revert: () => void,
  ): Promise<void> {
    if (!tokenIdResolved || !serverUrl) return
    const httpUrl = serverUrl.replace(/^ws/, 'http').replace(/\/ws\/session$/, '')
    setStreamingSaveBusy(true)
    try {
      await serverFetch(bridge, httpUrl, `/api/dashboard/tokens/${encodeURIComponent(tokenIdResolved)}/streaming-settings`, {
        method: 'PUT',
        body: JSON.stringify(next),
      })
    } catch {
      revert()
      showToast({ type: 'error', title: 'Setting not saved', message: 'Could not reach the server — reverted.' })
    } finally { setStreamingSaveBusy(false) }
  }

  return (
    <div class={styles.panel}>
      <SettingsSection title="Connection">
        <SettingsInput
          label="Server URL"
          type="text"
          placeholder="ws://localhost:3456"
          value={serverUrl}
          onChange={setServerUrl}
        />
        <SettingsInput
          label="Token"
          type="password"
          placeholder="Paste your token here"
          value={token}
          onChange={setToken}
          labelHint={tokenName ?? undefined}
        />
        <ConnectButton
          serverUrl={serverUrl}
          token={token}
          onTokenChange={setToken}
          whisperUrl={whisperUrl}
          whisperModel={whisperModel}
          whisperToken={whisperToken}
        />
      </SettingsSection>

      <WhisperSettings
        url={whisperUrl}
        model={whisperModel}
        token={whisperToken}
        onUrlChange={setWhisperUrl}
        onModelChange={setWhisperModel}
        onTokenChange={setWhisperToken}
      />

      {/* Theme picker, background-notification + ConPTY toggles moved to
          KaminIDE's native Settings page — they're host/app concerns, not Bridge
          conversation settings. */}

      <SettingsSection title="Base Prompt">
        <div style="font-size:12px;color:var(--text-muted);padding:0 0 8px">
          Standing instructions sent with every session on this token. They are
          ADDED to Bridge's own prompt (the one telling Claude your files live on
          your machine, not the server) — they never replace it, and your
          project's CLAUDE.md still applies on top. Takes effect on the next
          session.
        </div>
        <textarea
          value={basePrompt}
          placeholder="e.g. Always answer in Russian. Prefer podman over docker."
          rows={6}
          onInput={(e) => { setBasePrompt((e.target as HTMLTextAreaElement).value); setBasePromptSaved(false) }}
          style="width:100%;box-sizing:border-box;background:var(--bg-mantle);color:var(--text-primary);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);padding:8px 10px;font-family:var(--font-mono);font-size:12px;resize:vertical"
        />
        <div style="display:flex;align-items:center;gap:10px;padding-top:8px">
          <button
            type="button"
            class="btn"
            onClick={() => {
              void bridge.setBasePrompt(basePrompt).then(() => {
                setBasePromptSaved(true)
                showToast({ type: 'success', title: 'Base prompt saved', message: 'Applies to the next session.' })
              })
            }}
          >
            Save
          </button>
          {basePromptSaved && <span style="font-size:12px;color:var(--accent-green)">Saved</span>}
        </div>
      </SettingsSection>

      <SettingsSection title="Session Logs">
        <div style="font-size:12px;color:var(--text-muted);padding:0 0 8px">
          Download EVERY conversation transcript tied to this token into a folder
          you choose — one <code style="color:var(--text-secondary)">&lt;id&gt;.jsonl</code>
          per session, pulled in batches so large transcripts export reliably.
        </div>
        {token.trim()
          ? <ExportTranscriptsButton />
          : <div style="font-size:12px;color:var(--text-muted)">Enter a token above to export its logs.</div>}
      </SettingsSection>

      <SettingsSection title="Live Response Streaming">
        {!tokenIdResolved && (
          <div style="font-size:12px;color:var(--text-muted);padding:6px 0">
            Streaming preferences are per-user. Connect with a valid token to configure.
          </div>
        )}
        {tokenIdResolved && (
          <>
            <Switch
              checked={streamingEnabled}
              disabled={streamingLoading || streamingSaveBusy}
              onChange={(v) => {
                setStreamingEnabled(v)
                void persistStreaming({ enabled: v }, () => setStreamingEnabled(!v))
              }}
              label="Stream responses word-by-word"
              description={
                <>
                  Show assistant text as it arrives from Anthropic, like in CLI REPL.
                  Bridge runs a per-session local TLS proxy to capture API responses.
                </>
              }
            />
            {streamingEnabled && (
              <div style="margin-left:24px;border-left:2px solid var(--bg-surface);padding-left:14px;margin-top:6px">
                <Switch
                  checked={streamingShowThinking}
                  disabled={streamingLoading || streamingSaveBusy}
                  onChange={(v) => {
                    setStreamingShowThinking(v)
                    void persistStreaming({ showThinkingStream: v }, () => setStreamingShowThinking(!v))
                  }}
                  label="Stream thinking blocks"
                  description="Render extended-thinking content live as it generates."
                />
                <Switch
                  checked={streamingShowSide}
                  disabled={streamingLoading || streamingSaveBusy}
                  onChange={(v) => {
                    setStreamingShowSide(v)
                    void persistStreaming({ showSideQuests: v }, () => setStreamingShowSide(!v))
                  }}
                  label="Show side-quest streams"
                  description="Title generation, sub-agents, hook calls. Usually noise — off by default."
                />
              </div>
            )}
            <div style="margin-top:10px;padding:8px 10px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);font-size:11px;color:var(--text-muted);line-height:1.5">
              <i class="fas fa-shield" style="margin-right:6px;color:var(--accent-primary)" />
              The proxy listens on <code style="color:var(--text-secondary)">127.0.0.1</code>. CA cert is stored at <code style="color:var(--text-secondary)">~/.claude/open-claude-bridge/proxy-ca.pem</code> and trusted only by Bridge-spawned CLI processes (via <code>NODE_EXTRA_CA_CERTS</code>) — your system trust store is not modified. Settings apply to new sessions on next start.
            </div>
          </>
        )}
      </SettingsSection>
    </div>
  )
}
