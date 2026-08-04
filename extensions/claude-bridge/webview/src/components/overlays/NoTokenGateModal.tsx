import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import { hasToken, sidebarMode, activeCustomizePanel } from '../../signals/ui'

/**
 * First-run gate. Default UX: the token name is auto-filled from the current
 * machine's `<domain>/<user>` and the user just hits "Create token". Pasting
 * an existing token is available as a small secondary link.
 */
/**
 * Accept free-form server URLs and return a canonical `ws(s)://host[:port]`
 * WITHOUT the `/ws/session` suffix. ConnectionManager tacks that on when it
 * opens the socket — duplicating it here doubles the path and the WS hangs
 * in "Connecting" forever.
 * Supported shapes:
 *   host                                 → ws://host:3456
 *   host:port                            → ws://host:port
 *   http://host[:port][/path]            → ws://host[:port] (path dropped)
 *   https://host[:port][/path]           → wss://host[:port]
 *   ws(s)://host[:port][/anypath]        → ws(s)://host[:port]
 */
function normalizeServerUrl(input: string): string {
  const raw = input.trim()
  if (!raw) return ''
  const m = raw.match(/^([a-zA-Z][a-zA-Z0-9+\-.]*):\/\/(.+)$/)
  let scheme = ''
  let rest = raw
  if (m) {
    scheme = m[1]!.toLowerCase()
    rest = m[2]!
  }
  // Strip path — ConnectionManager appends /ws/session itself.
  const hostAndPort = rest.split(/[\/?#]/)[0] || rest
  const targetScheme = scheme === 'https' || scheme === 'wss'
    ? 'wss'
    : scheme === 'http' || scheme === 'ws'
      ? 'ws'
      : 'ws'
  // Default port 3456 when the host has no explicit port and is localhost-ish
  let finalHost = hostAndPort
  if (!/:\d+$/.test(finalHost) && /^(localhost|127\.0\.0\.1|0\.0\.0\.0)$/.test(finalHost)) {
    finalHost = `${finalHost}:3456`
  }
  return `${targetScheme}://${finalHost}`
}

/** Mirror the normalize logic for the HTTP base used by /api/dashboard/tokens. */
function httpBaseFromServerUrl(serverUrl: string): string {
  const canonical = normalizeServerUrl(serverUrl) || serverUrl
  return canonical
    .replace(/^wss:\/\//, 'https://')
    .replace(/^ws:\/\//, 'http://')
    .replace(/\/ws\/session$/, '')
}

export function NoTokenGateModal(): JSX.Element | null {
  const bridge = useBridge()
  const [serverUrl, setServerUrl] = useState('ws://localhost:3456/ws/session')
  const [machineId, setMachineId] = useState('')
  const [pasteMode, setPasteMode] = useState(false)
  const [token, setToken] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    bridge.getConfig().then((cfg: any) => {
      if (cfg?.serverUrl) setServerUrl(cfg.serverUrl)
    }).catch(() => { /* first-launch */ })
    bridge.getMachineId().then(setMachineId).catch(() => setMachineId('this-machine'))
  }, [])

  if (hasToken.value !== false) return null

  const canonicalUrl = normalizeServerUrl(serverUrl)
  const httpUrl = httpBaseFromServerUrl(serverUrl)

  async function handleCreate(): Promise<void> {
    if (!machineId) { setError('Machine id unavailable'); return }
    setBusy(true); setError(null)
    try {
      // Server is idempotent on `name`: if a token with this name already
      // exists (e.g. app reinstalled on the same machine), it returns the
      // existing token with HTTP 200 instead of issuing a duplicate.
      const name = machineId
      // Routed through the ext-host (Node) — the sandboxed webview can't fetch
      // the http server directly (CSP `connect-src 'self' https:`).
      const data = await (bridge as unknown as {
        createToken: (httpBase: string, name: string) => Promise<{ token: string; name: string }>
      }).createToken(httpUrl, name)
      if (!data?.token) throw new Error('Server did not return a token')
      // setConfig is fire-and-forget (ipcRenderer.send). Re-read to make sure
      // the main process actually persisted it before we drop the gate.
      await bridge.setConfig({ serverUrl: canonicalUrl, token: data.token })
      for (let i = 0; i < 10; i++) {
        const saved = await bridge.getConfig()
        if (saved?.token === data.token) break
        await new Promise(r => setTimeout(r, 50))
      }
      sidebarMode.value = 'sessions'
      activeCustomizePanel.value = null
      hasToken.value = true
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally { setBusy(false) }
  }

  async function handleSavePaste(): Promise<void> {
    if (!token.trim()) { setError('Token is required'); return }
    setBusy(true); setError(null)
    try {
      await bridge.setConfig({ serverUrl: canonicalUrl, token: token.trim() })
      for (let i = 0; i < 10; i++) {
        const saved = await bridge.getConfig()
        if (saved?.token === token.trim()) break
        await new Promise(r => setTimeout(r, 50))
      }
      sidebarMode.value = 'sessions'
      activeCustomizePanel.value = null
      hasToken.value = true
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally { setBusy(false) }
  }

  const overlayStyle = 'position:fixed;top:32px;left:0;right:0;bottom:0;background:var(--tint-overlay-heavy);backdrop-filter:blur(8px);z-index:9999;display:flex;align-items:center;justify-content:center;'
  const cardStyle = 'background:var(--bg-primary);border:1px solid var(--bg-overlay);border-radius:var(--radius-lg);padding:32px;width:480px;max-width:92vw;box-shadow:var(--shadow-modal);'
  const labelStyle = 'font-size:12px;color:var(--text-secondary);display:block;margin-bottom:6px;letter-spacing:0.3px;'
  const inputStyle = 'width:100%;padding:12px 14px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);color:var(--text-primary);font-size: var(--fs-md);outline:none;box-sizing:border-box;'
  const primaryBtn = 'padding:12px 18px;background:var(--accent-green);color:var(--bg-primary);border:none;border-radius:var(--radius-md);cursor:pointer;font-size:13px;font-weight:600;'
  const secondaryBtn = 'padding:12px 18px;background:transparent;border:1px solid var(--bg-overlay);color:var(--text-primary);border-radius:var(--radius-md);cursor:pointer;font-size:13px;'
  const linkStyle = 'background:none;border:none;padding:0;color:var(--accent-primary);cursor:pointer;font-size:11px;text-decoration:underline;'

  return (
    <div style={overlayStyle}>
      <div style={cardStyle}>
        <div style="display:flex;align-items:center;gap:12px;margin-bottom:8px;">
          <i class="fas fa-shield-halved" style="color:var(--accent-green);font-size:22px" />
          <div style="font-size: var(--fs-lg);font-weight:700;color:var(--text-primary)">Set up your bridge token</div>
        </div>
        <div style="color:var(--text-muted);font-size:13px;margin-bottom:20px;line-height:1.5">
          A token will be created on the server with your machine name. No manual input needed.
        </div>

        <div style="margin-bottom:14px">
          <label style={labelStyle}>Server URL</label>
          <input
            type="text"
            value={serverUrl}
            onInput={(e) => setServerUrl((e.target as HTMLInputElement).value)}
            placeholder="localhost:3456 · http://host · wss://host/ws/session"
            style={inputStyle}
          />
          {canonicalUrl && canonicalUrl !== serverUrl && (
            <div style="font-size:11px;color:var(--text-muted);margin-top:4px;font-family:ui-monospace,Consolas,monospace">
              → {canonicalUrl}
            </div>
          )}
        </div>

        {!pasteMode && (
          <>
            <div style="margin-bottom:18px">
              <label style={labelStyle}>Token name (auto from this machine)</label>
              <div style={`${inputStyle}display:flex;align-items:center;font-family:ui-monospace,Consolas,monospace;color:var(--accent-green)`}>
                {machineId || '…'}
              </div>
            </div>
            <div style="display:flex;gap:10px;justify-content:flex-end;align-items:center">
              <button
                style={primaryBtn}
                onClick={handleCreate}
                disabled={busy || !machineId}
              >
                {busy ? 'Creating…' : (
                  <>
                    <i class="fas fa-wand-magic-sparkles" style="margin-right:6px" />
                    Create token & continue
                  </>
                )}
              </button>
            </div>
            <div style="margin-top:14px;text-align:right">
              <button style={linkStyle} onClick={() => { setPasteMode(true); setError(null) }}>
                I already have a token — paste it instead
              </button>
            </div>
          </>
        )}

        {pasteMode && (
          <div>
            <label style={labelStyle}>Token</label>
            <input type="password" value={token} onInput={(e) => setToken((e.target as HTMLInputElement).value)}
              onKeyDown={(e) => { if (e.key === 'Enter') handleSavePaste() }}
              placeholder="ocb_..." autoFocus style={inputStyle} />
            <div style="display:flex;gap:10px;margin-top:16px;justify-content:flex-end">
              <button style={secondaryBtn} onClick={() => { setPasteMode(false); setError(null) }} disabled={busy}>Back</button>
              <button style={primaryBtn} onClick={handleSavePaste} disabled={busy || !token.trim()}>
                {busy ? 'Saving…' : 'Save & Continue'}
              </button>
            </div>
          </div>
        )}

        {error && (
          <div style="margin-top:14px;padding:10px 12px;border-radius:var(--radius-sm);border:1px solid var(--accent-red);background:var(--bg-tint-red);color:var(--accent-red);font-size:12px;display:flex;align-items:center;gap:8px">
            <i class="fas fa-circle-xmark" />
            <span>{error}</span>
          </div>
        )}
      </div>
    </div>
  )
}
