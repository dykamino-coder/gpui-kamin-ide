// The reported regression: text that shares a message with a tool_use stopped
// rendering in chat (splitProseFromTools split it into a prose bubble + a tool
// group). This runs the REAL render model (merge → group/split → visibility) and
// asserts the prose survives into visibleMerged with its text intact — the thing
// the eyeball can't see in a stripped diag.
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>

import { getRenderModel, dropRenderModelCache } from './derived-cache.ts'
import type { JsonlEntryData } from '../../types/jsonl'

let failures = 0
function assert(cond: boolean, msg: string): void {
  if (!cond) { failures++; console.error('  ✗ ' + msg) }
}
const textOf = (e: JsonlEntryData): string => {
  const c = (e as any).message?.content
  if (!Array.isArray(c)) return ''
  return c.filter((b: any) => b.type === 'text').map((b: any) => b.text).join('')
}

let uid = 0
const mixed = (text: string, tool: string): JsonlEntryData =>
  ({ type: 'assistant', uuid: `mix${uid}`, _ord: uid++ * 10, timestamp: `t${uid}`,
     message: { id: `m${uid}`, role: 'assistant', content: [
       { type: 'text', text }, { type: 'tool_use', id: `tu${uid}`, name: tool, input: {} },
     ] } }) as never
const toolResult = (): JsonlEntryData =>
  ({ type: 'user', uuid: `u${uid}`, _ord: uid++ * 10, message: { role: 'user',
     content: [{ type: 'tool_result', tool_use_id: `tu${uid}`, content: 'ok' }] } }) as never
const userMsg = (t: string): JsonlEntryData =>
  ({ type: 'user', uuid: `um${uid}`, _ord: uid++ * 10, message: { role: 'user', content: [{ type: 'text', text: t }] } }) as never

const TAB = 't1'
console.log('Scenario: prose that shares a message with a tool_use still renders')
{
  const entries: JsonlEntryData[] = [
    userMsg('do it'),
    mixed('Now I will rebuild the parser:', 'Bash'), toolResult(),
    mixed('And run the tests:', 'Bash'), toolResult(),
  ]
  dropRenderModelCache(TAB)
  const model = getRenderModel(TAB, entries, [], 0, 1)
  const proseTexts = model.visibleMerged.filter(e => e.type === 'assistant').map(textOf).filter(Boolean)
  console.log('    visibleMerged types:', model.visibleMerged.map(e => e.type).join(','))
  console.log('    prose texts found  :', JSON.stringify(proseTexts))
  assert(proseTexts.includes('Now I will rebuild the parser:'), 'first prose survives with its text')
  assert(proseTexts.includes('And run the tests:'), 'second prose survives with its text')
  // The tools still group.
  assert(model.visibleMerged.some(e => e.type === 'tool_group'), 'the tools form a group')
}

console.log('')
if (failures === 0) console.log('✅ PROSE-SPLIT RENDER OK')
else { console.error(`❌ ${String(failures)} FAILED — the split drops prose`); process.exit(1) }
