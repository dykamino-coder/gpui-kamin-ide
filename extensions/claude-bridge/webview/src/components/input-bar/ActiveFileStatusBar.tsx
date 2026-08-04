import type { JSX } from 'preact'
import { activeSelection, attachActiveFile } from '../../signals/file-viewer'
import { AttachActiveFileSwitch } from './AttachActiveFileSwitch'

function basename(p: string): string {
  return p.replace(/^.*[/\\]/, '')
}

/** Tiny status strip rendered above the InputBar showing the active
 *  file + caret/selection range. Visually de-emphasised so it doesn't
 *  steal attention from the textarea. Tints accent when the auto-attach
 *  switch is on so the user can see at a glance what would ride along
 *  with the next message. */
export function ActiveFileStatusBar(): JSX.Element | null {
  const sel = activeSelection.value
  if (!sel) return null
  const range = sel.startLine === sel.endLine
    ? `L${sel.startLine}`
    : `L${sel.startLine}-L${sel.endLine}`
  const lineCount = sel.endLine - sel.startLine + 1
  const willAttach = attachActiveFile.value

  return (
    <div
      data-tooltip={sel.path}
      style={`
        display:flex;align-items:center;gap:6px;
        padding:2px 10px;
        max-width:800px;width:100%;margin:0 auto;
        font-size:10px;color:${willAttach ? 'var(--accent-primary)' : 'var(--text-muted)'};
        font-family:ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace;
        white-space:nowrap;overflow:hidden;
        opacity:${willAttach ? 1 : 0.7};
        transition:color 140ms, opacity 140ms;
      `}
    >
      <i class={willAttach ? 'fas fa-paperclip' : 'fas fa-file-lines'} style="font-size:9px;flex-shrink:0" />
      <span style="overflow:hidden;text-overflow:ellipsis;min-width:0">{basename(sel.path)}</span>
      <span style="color:var(--text-disabled)">·</span>
      <span style="flex-shrink:0">{range}</span>
      {sel.text && (
        <>
          <span style="color:var(--text-disabled)">·</span>
          <span style="flex-shrink:0">{lineCount} {lineCount === 1 ? 'line' : 'lines'} · {sel.text.length} chars</span>
        </>
      )}
      <span style="margin-left:auto;flex-shrink:0;display:flex;align-items:center">
        <AttachActiveFileSwitch />
      </span>
    </div>
  )
}
