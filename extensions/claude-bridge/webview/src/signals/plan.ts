import { computed } from '@preact/signals'
import { jsonlEntriesByTab, getJsonlStructureVersion } from './jsonl'
import { getToolResults } from '../lib/tool-result-cache'
import { activeTabId } from './tabs'
import { stripMcpPrefix } from '../lib/mcp-tool-name'

// CLI stores only these three; `deleted` is a TaskUpdate action (tombstone),
// not a persisted status. Keep `cancelled`/`failed` for forward-compat in case
// a future CLI version extends the enum.
export type TaskStatus = 'pending' | 'in_progress' | 'completed' | 'cancelled' | 'failed'

export interface PlanTask {
  id: string
  subject: string
  description?: string
  activeForm?: string
  status: TaskStatus
  metadata?: Record<string, unknown>
  createdAt: string | undefined
  updatedAt: string | undefined
}

export interface PlanSnapshot {
  tasks: PlanTask[]
  total: number
  completed: number
  inProgress: number
  allDone: boolean
}

/** Walk the active tab's JSONL stream and rebuild the plan from
 *  `TaskCreate` / `TaskUpdate` calls.
 *
 *  IDs: CLI assigns task IDs via a persistent high-water-mark file, so a
 *  fresh session in an existing project can start at #20 or #25 — NOT #1.
 *  We can't simulate that counter, so we parse the real ID out of each
 *  TaskCreate's tool_result string (`Task #<id> created successfully: …`)
 *  and map tool_use.id → real id via a first pass. Any TaskCreate whose
 *  tool_result hasn't streamed in yet falls back to a sequential `live-N`
 *  placeholder; once the result arrives the computed re-runs and picks up
 *  the real id.
 *
 *  TaskUpdate can change subject/description/activeForm in addition to
 *  status; `status: 'deleted'` is a tombstone that removes the task
 *  entirely (see TaskUpdateTool.ts — deleteTask() returns early). */
export const activePlan = computed<PlanSnapshot | null>(() => {
  const id = activeTabId.value
  if (!id) return null
  const entries = jsonlEntriesByTab.value.get(id) ?? []

  // Pass 1: harvest tool_use_id → real task_id from the shared tool-result map.
  // It used to walk every user record's body here to do it — a third copy of the
  // same walk, re-run on every store change, 26ms per arriving record on a real
  // 31k session. The shared map differs in one way: it joins ALL text blocks of
  // a result where this pass read only the first. That can only ever find MORE
  // matches, never fewer, and on the real transcript both produce the same 61
  // task ids, so the fold stays authoritative.
  const toolUseIdToTaskId = new Map<string, string>()
  const taskCreatedRe = /Task #(\S+?) created successfully/
  const results = getToolResults(id, entries, getJsonlStructureVersion(id)).map
  results?.forEach((r, tuId) => {
    const m = r.content.match(taskCreatedRe)
    if (m) toolUseIdToTaskId.set(tuId, m[1]!)
  })

  // Pass 2: build plan in chronological order.
  const tasks: PlanTask[] = []
  const byId = new Map<string, PlanTask>()
  let livePlaceholder = 0

  for (const e of entries) {
    if (e.type !== 'assistant') continue
    const ts: string | undefined = (e as any).timestamp
    const content = (e as any).message?.content
    if (!Array.isArray(content)) continue
    for (const b of content) {
      if (b?.type !== 'tool_use') continue
      const shortName = stripMcpPrefix(String(b.name || ''))
      if (shortName === 'TaskCreate') {
        const toolUseId = typeof b.id === 'string' ? b.id : ''
        const resolved = toolUseId ? toolUseIdToTaskId.get(toolUseId) : undefined
        const idStr = resolved ?? `live-${++livePlaceholder}`
        const md = b.input?.metadata
        const t: PlanTask = {
          id: idStr,
          subject: String(b.input?.subject ?? b.input?.description ?? '(untitled)'),
          description: typeof b.input?.description === 'string' ? b.input.description : undefined,
          activeForm: typeof b.input?.activeForm === 'string' ? b.input.activeForm : undefined,
          status: 'pending',
          metadata: md && typeof md === 'object' && !Array.isArray(md) ? { ...md as Record<string, unknown> } : undefined,
          createdAt: ts,
          updatedAt: ts,
        }
        tasks.push(t)
        byId.set(idStr, t)
      } else if (shortName === 'TaskUpdate') {
        const tid = b.input?.taskId != null ? String(b.input.taskId) : null
        if (!tid || !byId.has(tid)) continue
        const existing = byId.get(tid)!
        const st = b.input?.status
        if (st === 'deleted') {
          const idx = tasks.indexOf(existing)
          if (idx >= 0) tasks.splice(idx, 1)
          byId.delete(tid)
          continue
        }
        if (typeof st === 'string') existing.status = st as TaskStatus
        if (typeof b.input?.subject === 'string') existing.subject = b.input.subject
        if (typeof b.input?.description === 'string') existing.description = b.input.description
        if (typeof b.input?.activeForm === 'string') existing.activeForm = b.input.activeForm
        // Merge metadata per CLI semantics: null-valued keys are deletes,
        // everything else overwrites. Missing input.metadata leaves it alone.
        const mdPatch = b.input?.metadata
        if (mdPatch && typeof mdPatch === 'object' && !Array.isArray(mdPatch)) {
          const merged: Record<string, unknown> = { ...(existing.metadata ?? {}) }
          for (const [k, v] of Object.entries(mdPatch as Record<string, unknown>)) {
            if (v === null) delete merged[k]
            else merged[k] = v
          }
          existing.metadata = Object.keys(merged).length > 0 ? merged : undefined
        }
        existing.updatedAt = ts ?? existing.updatedAt
      }
    }
  }

  if (tasks.length === 0) return null
  const completed = tasks.filter(t => t.status === 'completed').length
  const inProgress = tasks.filter(t => t.status === 'in_progress').length
  return {
    tasks,
    total: tasks.length,
    completed,
    inProgress,
    allDone: completed === tasks.length,
  }
})
