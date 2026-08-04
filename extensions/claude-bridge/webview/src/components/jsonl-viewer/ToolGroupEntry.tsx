import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import type { JsonlEntryData } from '../../types/jsonl'
import { toolNamesInEntry, prettyToolName } from './utils'
import { JsonlEntry } from './JsonlEntry'
import { ToolGroupSummary } from './ToolGroupSummary'
import { getAgentColor } from '../../utils/agent-color'
import { activeTabId, tabs } from '../../signals/tabs'

interface ToolGroupEntryProps {
  entry: JsonlEntryData
  toolResults?: Map<string, { content: string; isError: boolean }>
}

/** A run of consecutive tool-only ENTRIES (several assistant messages in a row
 *  that did nothing but call tools), collapsed into one block.
 *
 *  Shares its header with ToolUseGroup — the difference between "tools in one
 *  message" and "tools across messages" is ours, not the user's, and it used to
 *  leak out as two unrelated-looking blocks. What legitimately differs stays:
 *  this one spans several turns (so it can say so), carries the agent-lane
 *  accent + timestamp, and wears the assistant bubble shell so it fuses with
 *  the bubbles around it. */
export function ToolGroupEntry({ entry, toolResults }: ToolGroupEntryProps): JSX.Element {
  const [expanded, setExpanded] = useState(false)
  const groupEntries = entry._groupEntries as JsonlEntryData[]
  if (!groupEntries?.length) return <></>

  // Match the active session's assistant-accent color so the burst block
  // visually belongs to the same "agent lane" as the bubbles above and below it.
  const tab = tabs.value.find(t => t.id === activeTabId.value)
  const accent = getAgentColor(tab?.sessionTitle) || 'var(--accent-green)'

  // Count tool calls by name across all assistant entries in the group.
  const counts = new Map<string, number>()
  let totalTools = 0
  let assistantTurns = 0
  for (const e of groupEntries) {
    if (e.type !== 'assistant') continue
    assistantTurns++
    for (const name of toolNamesInEntry(e)) {
      const pretty = prettyToolName(name)
      counts.set(pretty, (counts.get(pretty) ?? 0) + 1)
      totalTools++
    }
  }
  const ranked = [...counts.entries()].sort((a, b) => b[1] - a[1])
  const lastTs = groupEntries[groupEntries.length - 1]?.timestamp

  // `tool-burst-expanded` lets global CSS strip inner assistant decorations so
  // nested tool-only entries look like continuous content, not cards in a card.
  return (
    <div
      class={`jsonl-entry jsonl-entry-assistant${expanded ? ' tool-burst-expanded' : ''}`}
      style={`border-left-color:${accent};--agent-color:${accent}`}
    >
      <ToolGroupSummary
        ranked={ranked}
        totalCalls={totalTools}
        turns={assistantTurns}
        expanded={expanded}
        onToggle={() => { setExpanded(!expanded) }}
        timestamp={lastTs}
        accent={accent}
      />
      {expanded && (
        <div class="tool-group-body">
          {groupEntries.map((e, i) => (
            <div key={e.uuid ?? `grp-${i}`}>
              <JsonlEntry entry={e} toolResults={toolResults} />
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
