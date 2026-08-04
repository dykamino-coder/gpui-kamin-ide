import type { JSX } from 'preact'
import type { JsonlEntryData, ContentBlock } from '../../types/jsonl'
import { hasCliXmlTags } from './utils'
import { CliXmlContent } from './CliXmlContent'
import { JsonlTimestamp } from './JsonlTimestamp'
import { MessageActions } from './MessageActions'

interface EditorContext {
  openedFile?: string
  selectedRange?: { line?: number; startLine?: number; endLine?: number }
  textInSelectedRange?: string | null
}

/** The InputBar appends a fenced ```json editor-context\n{...}\n``` block
 *  at the end of every message when the auto-attach switch is on (see
 *  useTabSend.ts). Strip it from the visible body and surface it as a
 *  compact chip in the header — the raw JSON in the bubble is noise to
 *  the user; the model still sees it because we only post-process the
 *  display, not the JSONL payload. */
const EDITOR_CTX_RE = /\n*```json editor-context\n([\s\S]*?)\n```\s*$/
function splitEditorContext(text: string): { body: string; ctx: EditorContext | null } {
  const m = EDITOR_CTX_RE.exec(text)
  if (!m) return { body: text, ctx: null }
  try {
    const ctx = JSON.parse(m[1]!) as EditorContext
    return { body: text.slice(0, m.index).replace(/\n+$/, ''), ctx }
  } catch {
    return { body: text, ctx: null }
  }
}

function basename(p: string): string { return p.replace(/^.*[/\\]/, '') }
function rangeLabel(r: EditorContext['selectedRange']): string {
  if (!r) return ''
  if (typeof r.line === 'number') return `L${r.line}`
  if (typeof r.startLine === 'number' && typeof r.endLine === 'number') {
    return r.startLine === r.endLine ? `L${r.startLine}` : `L${r.startLine}-L${r.endLine}`
  }
  return ''
}

export function UserEntry({ entry }: { entry: JsonlEntryData }): JSX.Element | null {
  const content = entry.message?.content
  let rawText = ''

  if (typeof content === 'string') {
    rawText = content
  } else if (Array.isArray(content)) {
    const textParts = content
      .filter((b: ContentBlock) => b.type === 'text')
      .map((b: ContentBlock) => b.text || '')
    rawText = textParts.join('\n') || JSON.stringify(content)
  } else {
    rawText = JSON.stringify(content)
  }

  // `interruptedByUser` is synthesised by the merge pass in JsonlViewer:
  // CLI emits `[Request interrupted by user]` as its OWN user entry
  // (utils/messages.ts:550 createInterruptedMessage) — visually that reads
  // as a duplicate user bubble. We collapse it into a compact badge on the
  // preceding user message, matching what CLI's <InterruptedByUser /> does
  // inline (components/InterruptedByUser.tsx) minus the ANSI styling.
  const interrupted = entry.interruptedByUser === true
  const skipped = entry.skippedByUser === true

  const { body: displayText, ctx: editorCtx } = splitEditorContext(rawText)
  const ctxFile = editorCtx?.openedFile
  const ctxRange = rangeLabel(editorCtx?.selectedRange)

  return (
    <div class="jsonl-entry jsonl-entry-user">
      <div class="entry-role">
        <i class="fas fa-user" style="font-size:10px" />
        {' '}User{' '}
        {entry.timestamp && <JsonlTimestamp timestamp={entry.timestamp} />}
        {ctxFile && (
          <span
            class="user-editor-context-badge"
            data-tooltip={`Sent with editor context: ${ctxFile}${ctxRange ? ` ${ctxRange}` : ''}${editorCtx?.textInSelectedRange ? ` · ${editorCtx.textInSelectedRange.length} chars selected` : ''}`}
            style="margin-left:8px;display:inline-flex;align-items:center;gap:4px;padding:1px 6px;border-radius:var(--radius-xs);background:var(--tint-primary-soft);color:var(--accent-primary);font-size:10px;font-weight:500;font-family:ui-monospace,'Cascadia Code',Menlo,Consolas,monospace;max-width:280px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;vertical-align:middle"
          >
            <i class="fas fa-paperclip" style="font-size:9px;flex-shrink:0" />
            <span style="overflow:hidden;text-overflow:ellipsis">{basename(ctxFile)}</span>
            {ctxRange && <span style="opacity:0.75;flex-shrink:0">· {ctxRange}</span>}
          </span>
        )}
        {interrupted && (
          <span
            class="user-interrupted-badge"
            title="Request was cancelled by Esc before the model finished"
            style="margin-left:8px;padding:1px 6px;border-radius:var(--radius-xs);background:var(--tint-orange-soft);color:var(--accent-orange-dark);font-size:10px;font-weight:600;letter-spacing:0.02em;text-transform:uppercase"
          >
            <i class="fas fa-ban" style="font-size:9px;margin-right:4px" />
            interrupted
          </span>
        )}
        {skipped && (
          <span
            class="user-skipped-badge"
            title="You skipped the AskUserQuestion prompt — model continued without your answer"
            style="margin-left:8px;padding:1px 6px;border-radius:var(--radius-xs);background:var(--tint-primary-medium);color:var(--accent-primary);font-size:10px;font-weight:600;letter-spacing:0.02em;text-transform:uppercase"
          >
            <i class="fas fa-forward" style="font-size:9px;margin-right:4px" />
            skipped
          </span>
        )}
        <MessageActions text={displayText || rawText} defaultName={`user-${entry.timestamp ?? Date.now()}.md`} />
      </div>
      {displayText && (hasCliXmlTags(displayText)
        ? <CliXmlContent text={displayText} />
        : <div class="entry-text">{displayText}</div>
      )}
    </div>
  )
}
