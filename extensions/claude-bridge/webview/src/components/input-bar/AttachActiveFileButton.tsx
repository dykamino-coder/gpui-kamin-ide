import { storage } from "@bridge/storage"
import type { JSX } from 'preact'
import { activeSelection, attachActiveFile, setAttachActiveFile } from '../../signals/file-viewer'

/** Toggle that auto-attaches the active file (and any selected lines)
 *  to every outgoing message. State is persisted to storage. The
 *  button shows live status — disabled when no file is open, accent-
 *  tinted when the toggle is on, and tooltip indicates which file +
 *  line range will be attached. */
export function AttachActiveFileButton(): JSX.Element {
  const on = attachActiveFile.value
  const sel = activeSelection.value
  const hasFile = !!sel
  const tooltip = !hasFile
    ? 'Open a file to enable file context attach'
    : on
      ? `Attach off — currently would send: ${sel.path}${
          sel.startLine === sel.endLine ? ` L${sel.startLine}` : ` L${sel.startLine}-L${sel.endLine}`
        }${sel.text ? ` (${sel.text.length} chars)` : ''}`
      : `Attach the active file + selection on send (${sel.path})`

  return (
    <button
      type="button"
      class="header-btn"
      onClick={() => setAttachActiveFile(!on)}
      data-tooltip={tooltip}
      aria-pressed={on}
      style={`background:${on ? 'color-mix(in srgb, var(--accent-primary) 14%, transparent)' : 'transparent'};color:${on ? 'var(--accent-primary)' : (hasFile ? 'var(--text-secondary)' : 'var(--text-disabled)')};cursor:${hasFile ? 'pointer' : 'default'}`}
    >
      <i class="fas fa-file-lines" style="font-size:13px" />
    </button>
  )
}
