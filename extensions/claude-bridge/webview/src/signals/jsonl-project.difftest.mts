// The tools panels keep only a projected slice of the jsonl store. This proves
// the projection is loss-free for what they render: activePlan and activeTodos
// computed over the FILTERED store must equal the same computed over the FULL
// store — otherwise a dropped entry silently empties the Plan/Todos panel.
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>

import { jsonlEntriesByTab } from './jsonl.ts'
import { activeTabId } from './tabs.ts'
import { activePlan } from './plan.ts'
import { activeTodos } from './todos.ts'
import { projectPlanTodoEntries } from './jsonl-project.ts'
import type { JsonlEntryData } from '../types/jsonl'

let failures = 0
function assert(cond: boolean, msg: string): void {
  if (!cond) { failures++; console.error('  ✗ ' + msg) }
}

let uid = 0
const asst = (blocks: unknown[]): JsonlEntryData =>
  ({ type: 'assistant', uuid: `a${uid++}`, timestamp: `t${uid}`, message: { role: 'assistant', content: blocks } }) as never
const user = (blocks: unknown[]): JsonlEntryData =>
  ({ type: 'user', uuid: `u${uid++}`, timestamp: `t${uid}`, message: { role: 'user', content: blocks } }) as never
const toolUse = (name: string, input: unknown, id = `tu${uid}`) => ({ type: 'tool_use', id, name, input })
const toolResult = (tuId: string, text: string) => ({ type: 'tool_result', tool_use_id: tuId, content: text })

// A realistic stream: plan/todo control entries buried in bulk noise.
const TAB = 'tab-1'
const entries: JsonlEntryData[] = [
  asst([{ type: 'text', text: 'Let me plan this.' }]),                       // noise: prose
  asst([toolUse('TaskCreate', { subject: 'Build parser', description: 'x' }, 'tc1')]),
  user([toolResult('tc1', 'Task #17 created successfully: Build parser')]),
  asst([toolUse('Read', { file_path: '/big/file' }, 'rd1')]),                // noise: Read
  user([toolResult('rd1', 'X'.repeat(50000))]),                             // noise: huge tool_result
  asst([{ type: 'thinking', thinking: 'hmm' }, toolUse('TaskUpdate', { taskId: '17', status: 'in_progress' }, 'tu2')]),
  asst([toolUse('Bash', { command: 'ls' }, 'bh1')]),                        // noise: Bash
  user([toolResult('bh1', 'file1\nfile2')]),                                // noise
  asst([toolUse('mcp__x__TaskCreate', { subject: 'Second' }, 'tc2')]),      // MCP-prefixed, still relevant
  user([toolResult('tc2', 'Task #18 created successfully: Second')]),
  asst([toolUse('TodoWrite', { todos: [{ content: 'do a', status: 'in_progress' }, { content: 'do b', status: 'pending' }] }, 'td1')]),
  asst([{ type: 'text', text: 'Done for now.' }]),                          // noise: prose
]

// --- Full store
activeTabId.value = TAB
jsonlEntriesByTab.value = new Map([[TAB, entries]])
const planFull = JSON.stringify(activePlan.value)
const todosFull = JSON.stringify(activeTodos.value)

// --- Filtered store (what the tools panels keep)
const filtered = projectPlanTodoEntries(entries)
jsonlEntriesByTab.value = new Map([[TAB, filtered]])
const planFiltered = JSON.stringify(activePlan.value)
const todosFiltered = JSON.stringify(activeTodos.value)

console.log(`projection kept ${String(filtered.length)}/${String(entries.length)} entries`)
assert(filtered.length < entries.length, 'projection actually drops noise')
assert(filtered.length === 6, `keeps exactly the 6 control entries (got ${String(filtered.length)})`)
assert(planFull === planFiltered, 'activePlan identical on filtered vs full store')
assert(todosFull === todosFiltered, 'activeTodos identical on filtered vs full store')
// Sanity: the plan really has both tasks with real ids, not placeholders.
assert(activePlan.value?.total === 2, `plan has 2 tasks (got ${String(activePlan.value?.total)})`)
assert(activeTodos.value?.total === 2, `todos has 2 items (got ${String(activeTodos.value?.total)})`)

console.log('')
if (failures === 0) console.log('✅ ALL PROJECTION CHECKS PASSED')
else { console.error(`❌ ${String(failures)} check(s) FAILED`); process.exit(1) }
