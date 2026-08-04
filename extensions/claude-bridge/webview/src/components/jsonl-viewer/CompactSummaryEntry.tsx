import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import type { JsonlEntryData } from '../../types/jsonl'
import { JsonlTimestamp } from './JsonlTimestamp'
import { renderMarkdown } from '../../utils/render-markdown'

interface ContentBlock {
  type: string
  text?: string
}

function extractText(entry: JsonlEntryData): string {
  const content = entry.message?.content
  if (typeof content === 'string') return content
  if (Array.isArray(content)) {
    return (content as ContentBlock[])
      .filter(b => b.type === 'text')
      .map(b => b.text || '')
      .join('\n\n')
  }
  return ''
}

export function CompactSummaryEntry({ entry }: { entry: JsonlEntryData }): JSX.Element | null {
  const [expanded, setExpanded] = useState(false)
  const text = extractText(entry).trim()
  if (!text) return null
  const wordCount = text.split(/\s+/).length
  const preview = text.slice(0, 180)

  return (
    <div class="jsonl-entry jsonl-entry-system">
      <div class="entry-role">
        <i class="fas fa-file-lines" style="font-size:10px;color:var(--text-secondary)" />
        {' '}
        <span style="color:var(--text-secondary);font-weight:600">Pre-compact summary</span>
        {' '}
        <span style="color:var(--text-disabled);font-size:11px;margin-left:6px">{wordCount.toLocaleString()} words</span>
        {' '}
        {entry.timestamp && <JsonlTimestamp timestamp={entry.timestamp} />}
      </div>
      <div style="padding:8px 12px;background:var(--bg-primary);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);margin-top:4px;color:var(--text-primary);font-size:12px">
        {expanded ? (
          <div dangerouslySetInnerHTML={{ __html: renderMarkdown(text) }} />
        ) : (
          <>
            <div style="color:var(--text-secondary);line-height:1.55;white-space:pre-wrap">{preview}{text.length > preview.length ? '…' : ''}</div>
          </>
        )}
        <button
          type="button"
          onClick={() => setExpanded(v => !v)}
          style="margin-top:8px;background:var(--bg-surface);color:var(--text-secondary);border:1px solid var(--bg-overlay);padding:4px 12px;border-radius:var(--radius-xs);font-size:11px;cursor:pointer"
        >
          {expanded ? 'Collapse' : 'Show full summary'}
        </button>
      </div>
    </div>
  )
}
