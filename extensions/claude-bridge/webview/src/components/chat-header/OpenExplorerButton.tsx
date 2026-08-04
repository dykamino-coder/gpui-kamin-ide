import type { JSX } from 'preact'
import { useBridge } from '../../hooks/useBridge'

interface OpenExplorerButtonProps {
  cwd: string
}

export function OpenExplorerButton({ cwd }: OpenExplorerButtonProps): JSX.Element {
  const bridge = useBridge()
  return (
    <button
      class="vscode-btn"
      data-tooltip="Open in file explorer"
      type="button"
      onClick={() => bridge.openFolder(cwd)}
    >
      <i class="fas fa-folder-open" style="font-size:13px;color:var(--text-primary)" />
    </button>
  )
}
