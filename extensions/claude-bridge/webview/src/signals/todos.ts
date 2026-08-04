import { computed } from '@preact/signals'
import { jsonlEntriesByTab } from './jsonl'
import { activeTabId } from './tabs'
import { stripMcpPrefix } from '../lib/mcp-tool-name'

// TodoWrite uses replace-semantics: each call overwrites the entire list. So
// the current state is whatever the LAST TodoWrite tool_use carried. We walk
// the JSONL entries chronologically and keep the most recent input.todos.
//
// This makes the panel survive PTY restart / model-effort change / container
// restart for free — same approach as activePlan. JSONL is the source of
// truth, in-memory caches in main process are just a fast path.

export type TodoStatus = 'pending' | 'in_progress' | 'completed'

export interface TodoItem {
  content: string
  status: TodoStatus
  activeForm?: string
}

export interface TodoSnapshot {
  items: TodoItem[]
  total: number
  completed: number
  inProgress: number
  allDone: boolean
  updatedAt?: string
}

export const activeTodos = computed<TodoSnapshot | null>(() => {
  const id = activeTabId.value
  if (!id) return null
  const entries = jsonlEntriesByTab.value.get(id) ?? []

  let lastTodos: TodoItem[] | null = null
  let lastTs: string | undefined

  // Backward, and stop at the first hit. Only the newest TodoWrite ever
  // survived — the forward version walked all 31k records of a real session to
  // overwrite its result 5 times, on every single store change. The last write
  // wins either way, so this is the same answer for a quarter of the work,
  // including the case where it yields an empty list (still the newest state).
  outer:
  for (let i = entries.length - 1; i >= 0; i--) {
    const e = entries[i]
    if (e.type !== 'assistant') continue
    const content = (e as any).message?.content
    if (!Array.isArray(content)) continue
    const ts: string | undefined = (e as any).timestamp
    for (let j = content.length - 1; j >= 0; j--) {
      const b = content[j]
      if (b?.type !== 'tool_use') continue
      if (stripMcpPrefix(String(b.name || '')) !== 'TodoWrite') continue
      const todos = b.input?.todos
      if (!Array.isArray(todos)) continue
      const items: TodoItem[] = []
      for (const t of todos) {
        if (!t || typeof t !== 'object') continue
        const tc = (t as any).content
        const ts2 = (t as any).status
        if (typeof tc !== 'string' || typeof ts2 !== 'string') continue
        if (ts2 !== 'pending' && ts2 !== 'in_progress' && ts2 !== 'completed') continue
        const af = (t as any).activeForm
        items.push({
          content: tc,
          status: ts2 as TodoStatus,
          activeForm: typeof af === 'string' ? af : undefined,
        })
      }
      lastTodos = items
      lastTs = ts
      break outer
    }
  }

  if (!lastTodos || lastTodos.length === 0) return null
  const completed = lastTodos.filter(t => t.status === 'completed').length
  const inProgress = lastTodos.filter(t => t.status === 'in_progress').length
  return {
    items: lastTodos,
    total: lastTodos.length,
    completed,
    inProgress,
    allDone: completed === lastTodos.length,
    updatedAt: lastTs,
  }
})
