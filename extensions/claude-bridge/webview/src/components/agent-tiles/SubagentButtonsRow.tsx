import type { JSX } from 'preact'
import { tabAgentTrees, fullscreenAgentId, listAgents } from '../../signals/agents'
import { activeTabId } from '../../signals/tabs'
import styles from './SubagentButtonsRow.module.css'

/** Compact chip row (one per subagent) shown just below the input. Replaces the
 *  always-open inline tiles — a subagent's turns no longer occupy the main chat;
 *  the user opens one on demand. Clicking a chip expands that agent's chat to
 *  fill the whole iframe (see SubagentFullscreen). Hidden when the session has
 *  no subagents. */
export function SubagentButtonsRow(): JSX.Element | null {
  const tabId = activeTabId.value
  const tree = tabId ? tabAgentTrees.value.get(tabId) : undefined
  // The chat's quick-access chips are for LIVE agents only — finished/kicked ones
  // stay in the Agents panel's tree + history, not cluttering the composer.
  const agents = listAgents(tree).filter((a) => a.status === 'running')
  if (agents.length === 0) return null
  return (
    <div class={styles.row} role="toolbar" aria-label="Subagents">
      {agents.map((a) => (
        <button
          key={a.name}
          type="button"
          class={styles.chip}
          onClick={() => { fullscreenAgentId.value = a.name }}
          title={`${a.name}${a.agentType ? ` · ${a.agentType}` : ''} — ${a.status}`}
        >
          <span class={`${styles.dot} ${styles[a.status]}`} aria-hidden="true" />
          <span class={styles.label}>{a.name}</span>
        </button>
      ))}
    </div>
  )
}
