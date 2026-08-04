import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import { updateInfo } from '../../signals/ui'

/** Compact version chip in the titlebar — sits between DevTools and
 *  the window controls. Shows `v6.2.49` in muted text by default;
 *  flips into a tinted action button (Restart-to-install / Downloading
 *  / Update / Retry) when the auto-updater reports a newer build is
 *  available. Behaviour mirrors the original sidebar VersionFooter so
 *  click semantics stay unchanged. */
export function TitlebarVersion(): JSX.Element | null {
  const bridge = useBridge()
  const [version, setVersion] = useState('')

  useEffect(() => {
    try { setVersion(bridge.getVersion()) } catch { /* version IPC missing during HMR */ }
  }, [bridge])

  if (!version) return null
  const info = updateInfo.value

  if (!info) {
    return (
      <span
        data-tooltip={`Open Claude Bridge v${version}`}
        style="display:inline-flex;align-items:center;padding:0 8px;font-size:10px;color:var(--text-disabled);font-family:ui-monospace,'Cascadia Code',Menlo,Consolas,monospace;-webkit-app-region:no-drag;letter-spacing:0.02em;cursor:default"
      >
        v{version}
      </span>
    )
  }

  const busy = info.state === 'downloading'
  const ready = info.state === 'ready'
  const errored = info.state === 'error'
  const label = ready
    ? `Restart to install ${info.serverVersion}`
    : busy
      ? `Downloading ${info.serverVersion}…`
      : errored
        ? `Update failed — retry`
        : `Update ${info.serverVersion}`
  const bg = ready ? 'var(--accent-green)' : errored ? 'var(--accent-red)' : 'var(--accent-primary)'

  async function onClick(): Promise<void> {
    if (ready) await bridge.quitAndInstallUpdate()
    else if (!busy) await bridge.applyUpdate()
  }

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={busy}
      data-tooltip={errored && info.error ? info.error : `Currently v${version}`}
      style={`display:inline-flex;align-items:center;gap:6px;padding:3px 10px;margin:0 4px;border:none;border-radius:var(--radius-sm);background:${bg};color:var(--bg-primary);font-size:10px;font-weight:600;cursor:${busy ? 'default' : 'pointer'};opacity:${busy ? 0.7 : 1};-webkit-app-region:no-drag;font-family:inherit;letter-spacing:0.02em;height:22px`}
    >
      <i class={ready ? 'fas fa-rotate-right' : busy ? 'fas fa-spinner fa-spin' : errored ? 'fas fa-triangle-exclamation' : 'fas fa-arrow-up'} style="font-size:9px" />
      {label}
    </button>
  )
}
