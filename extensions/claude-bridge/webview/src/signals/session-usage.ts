import { signal } from '@preact/signals'
import type { AgentEngine, SessionUsage } from '../../shared/types'

// Live subscription limits (5-hour / 7-day) and context fill per tab, pushed by
// the server from the CLI status line (`session:usage`). Own module, like
// replay-progress.ts, so pure-signal tests do not pull ui.ts storage.
export const usageByTab = signal<Map<string, SessionUsage>>(new Map())

export function setTabUsage(tabId: string, usage: SessionUsage | null): void {
  const cur = usageByTab.value
  if (usage === null) {
    if (!cur.has(tabId)) return
    const next = new Map(cur); next.delete(tabId); usageByTab.value = next
    return
  }
  const next = new Map(cur); next.set(tabId, usage); usageByTab.value = next
}

/** The agent a usage snapshot describes. Servers that do not tag it only
 *  send the Claude CLI status line, so an untagged snapshot is Claude's. */
export function usageEngine(usage: SessionUsage): AgentEngine {
  return usage.engine === 'codex' ? 'codex' : 'claude'
}

/** The tab's usage when it belongs to the agent the chat runs now. A chat that
 *  moved to the other agent must not show the previous agent's plan windows or
 *  auto-compact point, including a late snapshot of the old one. */
export function usageForEngine(usage: SessionUsage | undefined, engine: AgentEngine | undefined): SessionUsage | undefined {
  if (!usage) return undefined
  return usageEngine(usage) === (engine ?? 'claude') ? usage : undefined
}

/** The chat moved to `engine`: drop a snapshot of the other agent. One of the
 *  new agent is kept — it can arrive before the switch is announced. */
export function forgetOtherEngineUsage(tabId: string, engine: AgentEngine): void {
  const usage = usageByTab.value.get(tabId)
  if (usage && usageEngine(usage) !== engine) setTabUsage(tabId, null)
}
