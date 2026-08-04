import type { JSX } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import { fullscreenAgentId, subagentTileState, tabAgentTrees, findAgentByName, agentEntriesWithLive } from '../../signals/agents'
import { jsonlEntriesByTab } from '../../signals/jsonl'
import { activeTabId } from '../../signals/tabs'
import { JsonlEntry } from '../jsonl-viewer/JsonlEntry'
import type { JsonlEntryData } from '../../types/jsonl'
import styles from './SubagentFullscreen.module.css'

/** The selected subagent's chat, expanded to fill the whole iframe over the main
 *  chat. Reads the same per-agent entry stream the tiles used (subagentTileState)
 *  and renders it with the main chat's own JsonlEntry, so a subagent turn looks
 *  identical to a main turn — just scoped to that agent. A Back button returns to
 *  the main chat. Renders nothing when no agent is expanded. */
export function SubagentFullscreen(): JSX.Element | null {
  const name = fullscreenAgentId.value
  const tabId = activeTabId.value
  const tree = tabId ? tabAgentTrees.value.get(tabId) : undefined
  // Subscribe to BOTH stores so canonical tile entries AND the live streaming
  // stub repaint the view. Read unconditionally to keep hook order stable.
  const tick = subagentTileState.value.size + (tabId ? (jsonlEntriesByTab.value.get(tabId)?.length ?? 0) : 0)
  void tick
  const info = name ? findAgentByName(tree, name) : undefined
  const entries = name ? (agentEntriesWithLive(tabId, name, info?.agentType) as JsonlEntryData[]) : []

  // Switching sessions must drop the overlay — the expanded agent belongs to the
  // tab we just left. (Hook runs before the early return so order stays stable.)
  const lastTabRef = useRef(tabId)
  useEffect(() => {
    if (lastTabRef.current !== tabId) { lastTabRef.current = tabId; fullscreenAgentId.value = null }
  }, [tabId])

  const bodyRef = useRef<HTMLDivElement | null>(null)
  const prevLenRef = useRef(0)
  useEffect(() => {
    const el = bodyRef.current
    if (!el) return
    if (entries.length > prevLenRef.current) {
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 60
      if (atBottom) el.scrollTop = el.scrollHeight
    }
    prevLenRef.current = entries.length
  }, [entries.length])

  if (!name) return null
  const close = (): void => { fullscreenAgentId.value = null }
  return (
    <div class={styles.overlay} role="dialog" aria-label={`Subagent ${name}`}>
      <div class={styles.header}>
        <button type="button" class={styles.back} onClick={close}>
          <i class="fas fa-arrow-left" aria-hidden="true" />
          <span>Back</span>
        </button>
        <span class={styles.title}>
          {name}{info?.agentType ? <span class={styles.type}> · {info.agentType}</span> : null}
        </span>
        {info?.status && <span class={`${styles.status} ${styles[info.status]}`}>{info.status}</span>}
      </div>
      <div class={styles.body} ref={bodyRef}>
        {entries.length === 0
          ? <div class={styles.empty}>No messages from this subagent yet.</div>
          : entries.map((entry, i) => <JsonlEntry key={i} entry={entry} />)}
      </div>
    </div>
  )
}
