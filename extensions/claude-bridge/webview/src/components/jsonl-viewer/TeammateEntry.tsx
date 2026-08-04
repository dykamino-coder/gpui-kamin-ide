import type { JSX } from 'preact'
import type { JsonlEntryData, ContentBlock } from '../../types/jsonl'
import { CliXmlContent } from './CliXmlContent'
import { JsonlTimestamp } from './JsonlTimestamp'

function getEntryText(entry: JsonlEntryData): string {
  const content = entry.message?.content
  if (typeof content === 'string') return content
  if (Array.isArray(content)) {
    return (content as ContentBlock[])
      .filter(b => b.type === 'text')
      .map(b => b.text || '')
      .join('\n')
  }
  return ''
}

export function TeammateEntry({ entry }: { entry: JsonlEntryData }): JSX.Element {
  const text = getEntryText(entry)

  return (
    <div class="jsonl-entry jsonl-entry-teammate">
      <div class="entry-role" style="font-size:11px;color:var(--accent-purple);margin-bottom:4px;display:flex;align-items:center;gap:6px">
        <i class="fas fa-users" style="font-size:10px" />
        {' '}Teammate
        {entry.timestamp && <JsonlTimestamp timestamp={entry.timestamp} />}
      </div>
      <CliXmlContent text={text} />
    </div>
  )
}
