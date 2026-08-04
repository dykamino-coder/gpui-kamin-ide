import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import { OpenVscodeButton } from './OpenVscodeButton'

interface Props {
  cwd?: string
  folderName?: string
  vscodeAvailable: boolean
}

/** Folder breadcrumb chip — replaces the old path-row.
 *  Click on the text/icon → open in OS file explorer.
 *  Click on the embedded VS Code icon → open in VS Code.
 *  Each interactive area is its own <button> so events don't bubble
 *  weirdly through nested click handlers. */
export function FolderCrumb({ cwd, folderName, vscodeAvailable }: Props): JSX.Element | null {
  const bridge = useBridge()
  const [exists, setExists] = useState<boolean | null>(null)

  useEffect(() => {
    if (!cwd) { setExists(null); return }
    let cancelled = false
    bridge.pathExists(cwd)
      .then(ok => { if (!cancelled) setExists(ok) })
      .catch(() => { if (!cancelled) setExists(false) })
    return () => { cancelled = true }
  }, [cwd])

  if (!cwd) return null

  const label = folderName || cwd.split(/[\\/]/).filter(Boolean).pop() || 'folder'
  const missing = exists === false
  const labelColor = missing ? 'var(--accent-red)' : 'var(--text-secondary)'
  const iconColor = missing ? 'var(--accent-red)' : 'var(--text-muted)'

  return (
    <span
      class="folder-crumb"
      style="display:inline-flex;align-items:center;gap:4px;padding:2px;border-radius:var(--radius-md);background:var(--bg-base);font-size:11px;font-family:'Cascadia Code',monospace;flex-shrink:0;max-width:260px;height:26px"
    >
      <button
        type="button"
        data-tooltip={missing ? `${cwd}\n(not on disk)` : `${cwd}\nOpen in Explorer`}
        onClick={() => bridge.openFolder?.(cwd)}
        style={`display:inline-flex;align-items:center;gap:6px;padding:2px 6px 2px 8px;border:none;background:transparent;border-radius:var(--radius-md);color:${labelColor};font-family:inherit;font-size:inherit;cursor:pointer;min-width:0`}
      >
        <i class={`fas ${missing ? 'fa-triangle-exclamation' : 'fa-folder'}`} style={`font-size:10px;color:${iconColor}`} />
        <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{label}</span>
      </button>
      {vscodeAvailable && !missing && <OpenVscodeButton cwd={cwd} />}
    </span>
  )
}
