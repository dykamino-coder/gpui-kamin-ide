import type { JSX } from 'preact'
import { useState } from 'preact/hooks'

interface WriteInput {
  file_path?: string
  content?: string
}

const PREVIEW_LINES = 40

/** Write tool: show file path + the content as a monospace block. Large
 *  files truncate to PREVIEW_LINES and expose a "Show full" toggle so
 *  the transcript doesn't get swamped by a 5K-line write. */
export function WriteToolRender({ input }: { input: WriteInput }): JSX.Element {
  const [expanded, setExpanded] = useState(false)
  const content = input.content ?? ''
  const lines = content.split('\n')
  const truncated = !expanded && lines.length > PREVIEW_LINES
  const shown = truncated ? lines.slice(0, PREVIEW_LINES).join('\n') : content

  return (
    <div style="padding:6px 0">
      {input.file_path && (
        <div style="font-family:var(--font-mono);font-size:11px;color:var(--accent-primary);padding:4px 10px;background:var(--bg-primary);border:1px solid var(--bg-surface);border-radius:var(--radius-xs) var(--radius-xs) 0 0;border-bottom:none;display:flex;align-items:center;gap:8px">
          <i class="fas fa-file-lines" style="color:var(--accent-green);font-size:10px" />
          {input.file_path}
          <span style="margin-left:auto;color:var(--text-muted);font-size:10px">{lines.length} lines</span>
        </div>
      )}
      <pre style="margin:0;padding:6px 10px;background:var(--bg-sidebar);color:var(--text-primary);font-family:var(--font-mono);font-size:11px;line-height:1.5;white-space:pre-wrap;word-break:break-word;border:1px solid var(--bg-surface);border-radius:0 0 var(--radius-xs) var(--radius-xs);overflow-x:auto">
        {shown}
        {truncated && '\n...'}
      </pre>
      {lines.length > PREVIEW_LINES && (
        <button
          onClick={() => setExpanded(!expanded)}
          type="button"
          style="margin-top:4px;background:transparent;border:1px solid var(--bg-overlay);color:var(--text-secondary);padding:4px 10px;border-radius:var(--radius-xs);cursor:pointer;font-size:10px"
        >
          {expanded ? `Collapse (${PREVIEW_LINES} lines)` : `Show full (${lines.length - PREVIEW_LINES} more lines)`}
        </button>
      )}
    </div>
  )
}
