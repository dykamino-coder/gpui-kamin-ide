import { storage } from "@bridge/storage"
import type { JSX } from 'preact'
import { activeSelection, attachActiveFile, setAttachActiveFile } from '../../signals/file-viewer'

/** iOS-style switch placed in the InputControls row (after the
 *  permissions / bypass dropdown) that toggles whether outgoing
 *  messages auto-attach the active file path + selected lines. State
 *  persists to localStorage via setAttachActiveFile. Disabled when
 *  no file is open. */
export function AttachActiveFileSwitch(): JSX.Element {
  const on = attachActiveFile.value
  const sel = activeSelection.value
  const hasFile = !!sel
  const tooltip = !hasFile
    ? 'Open a file to enable file context attach'
    : on
      ? `Auto-attach: ${sel.path}${
          sel.startLine === sel.endLine ? ` L${sel.startLine}` : ` L${sel.startLine}-L${sel.endLine}`
        }`
      : `Click to auto-attach the active file + selection on every send`

  return (
    <button
      type="button"
      class="attach-file-switch"
      onClick={() => { if (hasFile || on) setAttachActiveFile(!on) }}
      data-tooltip={tooltip}
      aria-pressed={on}
      disabled={!hasFile && !on}
      style={`
        display:inline-flex;align-items:center;gap:6px;
        padding:2px 8px 2px 6px;border:none;background:transparent;
        color:${on ? 'var(--accent-primary)' : 'var(--text-muted)'};
        font-size:11px;font-weight:500;
        cursor:${(hasFile || on) ? 'pointer' : 'default'};
        opacity:${(hasFile || on) ? 1 : 0.5};
      `}
    >
      <span
        aria-hidden="true"
        style={`
          position:relative;width:24px;height:14px;border-radius:8px;
          background:${on ? 'var(--accent-primary)' : 'var(--bg-overlay)'};
          transition:background 140ms ease;
          flex-shrink:0;
        `}
      >
        <span
          style={`
            position:absolute;top:2px;left:${on ? '12px' : '2px'};
            width:10px;height:10px;border-radius:50%;
            background:var(--bg-primary);
            transition:left 140ms cubic-bezier(.4,.0,.2,1);
            box-shadow:0 1px 2px rgba(0,0,0,0.25);
          `}
        />
      </span>
      <span>attach file</span>
    </button>
  )
}
