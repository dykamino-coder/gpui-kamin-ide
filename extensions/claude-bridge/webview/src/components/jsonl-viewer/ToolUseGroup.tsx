import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import type { ContentBlock } from '../../types/jsonl'
import { JsonlToolUse } from './JsonlToolUse'
import { ToolGroupSummary } from './ToolGroupSummary'
import { prettyToolName } from './utils'

interface ToolUseGroupProps {
  /** Precondition (owned by the caller in ContentBlocks): a run of ≥2 blocks. */
  blocks: ContentBlock[]
  toolResults?: Map<string, { content: string; isError: boolean }>
}

/** A run of consecutive tool_use blocks WITHIN one assistant message, collapsed
 *  into a "3 tool calls · Read ×3" summary — parallel tool calls under a text
 *  turn ("Let me read all of them" + 3× Read) read as one tidy group.
 *
 *  Shares its header with ToolGroupEntry (which groups whole tool-only ENTRIES):
 *  same event from the user's side, so the same look. No `turns` here — these
 *  blocks are one message by definition, and "· 1 turns" says nothing. */
export function ToolUseGroup({ blocks, toolResults }: ToolUseGroupProps): JSX.Element {
  const [expanded, setExpanded] = useState(false)
  const counts = new Map<string, number>()
  for (const b of blocks) {
    const pretty = prettyToolName(b.name || 'unknown')
    counts.set(pretty, (counts.get(pretty) ?? 0) + 1)
  }
  const ranked = [...counts.entries()].sort((a, b) => b[1] - a[1])

  return (
    <div class={`block-tool-group ${expanded ? 'expanded' : ''}`}>
      <ToolGroupSummary
        ranked={ranked}
        totalCalls={blocks.length}
        expanded={expanded}
        onToggle={() => { setExpanded(!expanded) }}
      />
      {expanded && (
        <div class="tool-group-body">
          {blocks.map((b, i) => (
            <JsonlToolUse
              key={i}
              toolName={b.name || 'unknown'}
              toolUseId={b.id || ''}
              input={b.input}
              result={b.id ? toolResults?.get(b.id) : undefined}
            />
          ))}
        </div>
      )}
    </div>
  )
}
