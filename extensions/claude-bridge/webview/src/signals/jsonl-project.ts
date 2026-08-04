// Projection filter for the NON-chat panels.
//
// Every Bridge panel (chat / plan / todos / agents / console / the 10 customize
// sections) runs the same listener and used to accumulate the FULL jsonl store —
// ~1GB on a large session, PER PANEL. A freeze report caught three panels each
// holding ~1.17GB (console/chat/plan); that duplication is the root of the OOM
// crashes and the memory-pressure "Not Responding" on the slow work machine.
//
// Only the CHAT panel renders the jsonl viewer. The tools panels derive plan +
// todos from the stream — but activePlan/activeTodos read only a tiny slice of
// it: assistant `TaskCreate`/`TaskUpdate`/`TodoWrite` tool calls, plus the user
// `tool_result` that carries the real task id ("Task #N created successfully").
// The agent tree is built incrementally (parseAgentEntries), not from the store,
// so it doesn't need this. Keeping only that slice makes the tools store a
// handful of small entries instead of the whole conversation.
//
// The filter MUST be a superset of what plan.ts / todos.ts read — dropping a
// relevant entry silently empties the panel. The difftest asserts filtered vs
// full produce identical activePlan/activeTodos.

import type { JsonlEntryData } from '../types/jsonl'
// Shared with plan.ts / todos.ts — matching on the BARE tool name via one
// source keeps this projection from ever drifting out of sync with what they
// read (a drift would silently empty the Plan/Todos panels).
import { stripMcpPrefix } from '../lib/mcp-tool-name'

const PLAN_TODO_TOOLS = new Set(['TaskCreate', 'TaskUpdate', 'TodoWrite'])

/** True when this entry can contribute to activePlan / activeTodos. Everything
 *  else — assistant prose, thinking, Read/Bash/Edit tool calls, and their
 *  (often huge) tool_results — is dropped from the non-chat store. */
export function isPlanTodoEntry(entry: JsonlEntryData): boolean {
  const e = entry as { type?: string; message?: { content?: unknown } }
  const content = e.message?.content
  if (!Array.isArray(content)) return false

  if (e.type === 'assistant') {
    for (const b of content as { type?: string; name?: unknown }[]) {
      if (b?.type === 'tool_use' && PLAN_TODO_TOOLS.has(stripMcpPrefix(String(b.name ?? '')))) return true
    }
    return false
  }

  if (e.type === 'user') {
    // The task-id mapping in plan.ts reads the "Task #<id> created successfully"
    // tool_result. Keep any user entry whose tool_result text mentions a task id;
    // a cheap superset of the exact regex, and these results are tiny.
    for (const b of content as { type?: string; content?: unknown }[]) {
      if (b?.type !== 'tool_result') continue
      const c = b.content
      let text = ''
      if (typeof c === 'string') text = c
      else if (Array.isArray(c)) {
        for (const inner of c as { type?: string; text?: unknown }[]) {
          if (inner?.type === 'text' && typeof inner.text === 'string') { text = inner.text; break }
        }
      }
      if (text.includes('Task #')) return true
    }
    return false
  }

  return false
}

/** Keep only the plan/todo-relevant entries from a batch. */
export function projectPlanTodoEntries(entries: JsonlEntryData[]): JsonlEntryData[] {
  return entries.filter(isPlanTodoEntry)
}
