// Copy-on-click monospace path chip extracted from SyncedDataPanel.tsx
// (Sprint 5 / Stage E1). Used for the "Sync base" + "Token dir" rows.

import type { JSX } from 'preact'
import { copyWithToast } from '../../../lib/clipboard'

export function SyncPathRow({ label, value }: { label: string; value: string }): JSX.Element {
  async function copy(): Promise<void> {
    await copyWithToast(value, 'Path copied')
  }
  return (
    <div style="display:flex;align-items:center;gap:8px;padding:4px 10px;background:var(--bg-primary);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);margin-bottom:6px;font-family:ui-monospace,Consolas,monospace;font-size:11px">
      <span style="color:var(--text-muted)">{label}:</span>
      <span style="color:var(--text-primary);flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title={value}>{value}</span>
      <button
        type="button"
        onClick={copy}
        data-tooltip="Copy"
        style="background:var(--bg-surface);border:1px solid var(--bg-overlay);color:var(--text-primary);padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;cursor:pointer"
      >
        <i class="fas fa-copy" />
      </button>
    </div>
  )
}
