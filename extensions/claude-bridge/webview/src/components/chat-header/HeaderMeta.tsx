import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { CwdDisplay } from './CwdDisplay'
import { OpenVscodeButton } from './OpenVscodeButton'
import { OpenExplorerButton } from './OpenExplorerButton'
import { useBridge } from '../../hooks/useBridge'

interface HeaderMetaProps {
  cwd?: string
  vscodeAvailable: boolean
}

export function HeaderMeta({ cwd, vscodeAvailable }: HeaderMetaProps): JSX.Element {
  const bridge = useBridge()
  const [exists, setExists] = useState<boolean | null>(null)

  useEffect(() => {
    if (!cwd) { setExists(null); return }
    let cancelled = false
    bridge.pathExists(cwd)
      .then((ok) => { if (!cancelled) setExists(ok) })
      .catch(() => { if (!cancelled) setExists(false) })
    return () => { cancelled = true }
  }, [cwd])

  if (!cwd) {
    return (
      <div class="chat-header-meta">
        <span class="meta-cwd" style="color:var(--text-muted);font-style:italic">Folder not set for this session</span>
      </div>
    )
  }

  if (exists === false) {
    return (
      <div class="chat-header-meta">
        <CwdDisplay cwd={cwd} />
        <span style="color:var(--accent-red);font-size:11px;margin-left:8px" data-tooltip="Path doesn't exist on this machine (container-only or moved)">
          <i class="fas fa-triangle-exclamation" /> not on disk
        </span>
      </div>
    )
  }

  return (
    <div class="chat-header-meta">
      <CwdDisplay cwd={cwd} />
      {vscodeAvailable && exists !== null && <OpenVscodeButton cwd={cwd} />}
      {exists !== null && <OpenExplorerButton cwd={cwd} />}
    </div>
  )
}
