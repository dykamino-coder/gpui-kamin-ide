// Все применения одного тула, раскрытые поверх чата (аналог
// SubagentFullscreen): assistant-записи, содержащие tool_use этого тула, +
// спаренные tool_result по tool_use_id. Рендер тем же JsonlEntry.
import type { JSX } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import { fullscreenToolName } from '../../signals/tool-usage'
import { jsonlEntriesByTab } from '../../signals/jsonl'
import { activeTabId } from '../../signals/tabs'
import { stripMcpPrefix } from '../../lib/mcp-tool-name'
import { JsonlEntry } from '../jsonl-viewer/JsonlEntry'
import { JsonlToolIcon } from '../jsonl-viewer/JsonlToolIcon'
import type { JsonlEntryData } from '../../types/jsonl'
import styles from '../agent-tiles/SubagentFullscreen.module.css'

function collectToolEntries(tabId: string | null, tool: string): JsonlEntryData[] {
  if (!tabId) return []
  const store = (jsonlEntriesByTab.value.get(tabId) ?? []) as Array<{
    type?: string
    message?: { content?: unknown }
  }>
  const useIds = new Set<string>()
  const out: JsonlEntryData[] = []
  for (const e of store) {
    const content = e.message?.content
    if (!Array.isArray(content)) continue
    if (e.type === 'assistant') {
      let hit = false
      for (const b of content as Array<{ type?: string; id?: string; name?: unknown }>) {
        if (b?.type === 'tool_use' && typeof b.name === 'string' && stripMcpPrefix(b.name) === tool) {
          if (b.id) useIds.add(b.id)
          hit = true
        }
      }
      if (hit) out.push(e as JsonlEntryData)
    } else if (e.type === 'user') {
      for (const b of content as Array<{ type?: string; tool_use_id?: string }>) {
        if (b?.type === 'tool_result' && b.tool_use_id && useIds.has(b.tool_use_id)) {
          out.push(e as JsonlEntryData)
          break
        }
      }
    }
  }
  return out
}

export function ToolUsageFullscreen(): JSX.Element | null {
  const tool = fullscreenToolName.value
  const tabId = activeTabId.value
  // Подписка на стор (repaint при новых записях), до early-return — порядок хуков.
  void jsonlEntriesByTab.value

  const lastTabRef = useRef(tabId)
  useEffect(() => {
    if (lastTabRef.current !== tabId) { lastTabRef.current = tabId; fullscreenToolName.value = null }
  }, [tabId])

  if (!tool) return null
  const entries = collectToolEntries(tabId, tool)
  const close = (): void => { fullscreenToolName.value = null }
  return (
    <div class={styles.overlay} role="dialog" aria-label={`Tool ${tool} calls`}>
      <div class={styles.header}>
        <button type="button" class={styles.back} onClick={close}>
          <i class="fas fa-arrow-left" aria-hidden="true" />
          <span>Back</span>
        </button>
        <span class={styles.title} style="display:inline-flex;align-items:center;gap:8px">
          <JsonlToolIcon toolName={tool} />
          {tool}
          <span class={styles.type}> · {entries.length} entries in window</span>
        </span>
      </div>
      <div class={styles.body}>
        {entries.length === 0
          ? <div class={styles.empty}>No calls of this tool in the loaded window.</div>
          : entries.map((entry, i) => <JsonlEntry key={i} entry={entry} />)}
      </div>
    </div>
  )
}
