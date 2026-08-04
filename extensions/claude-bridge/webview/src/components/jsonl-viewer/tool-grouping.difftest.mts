// A bookkeeping row must never split a tool burst, hide real messages, or count.
//
// THE BUG: `JsonlEntry` renders exactly user/assistant/system/attachment/
// tool_group — everything else returns null. Any type that renders null but
// isn't in NON_RENDERING_ENTRY_TYPES therefore counts as "visible" while drawing
// nothing, and does three kinds of damage at once. `mode` (542 of them in a real
// session) was missing from the list, and the user hit all three:
//   1. two tool-burst blocks back to back where there should be one
//   2. the render window filling with rows that draw nothing (blank chat)
//   3. inflated counts on the segment tab pills
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>

import { groupConsecutiveToolCalls, NON_RENDERING_ENTRY_TYPES, isVisibleAttachment, isToolOnlyAssistantEntry } from './utils.ts'
import { entryIsVisible } from './prepare-jsonl-entries.ts'
import type { JsonlEntryData } from '../../types/jsonl'

let failures = 0
function assert(cond: boolean, msg: string): void {
  if (!cond) { failures++; console.error('  ✗ ' + msg) }
}

let uid = 0
/** An assistant entry whose content is ONLY tool_use — what a burst is made of. */
const toolCall = (name: string): JsonlEntryData =>
  ({ type: 'assistant', uuid: `a${uid++}`, message: { id: `m${uid}`, role: 'assistant', content: [{ type: 'tool_use', id: `t${uid}`, name, input: {} }] } }) as never
const toolResult = (): JsonlEntryData =>
  ({ type: 'user', uuid: `u${uid++}`, message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: `t${uid}`, content: 'ok' }] } }) as never
const bookkeeping = (type: string): JsonlEntryData => ({ type, uuid: `k${uid++}` }) as never
/** Prose and a tool_use in the SAME assistant message — what the model emits
 *  when it narrates before acting. Fixed uuid so the split can be asserted. */
const mixed = (text: string, tool: string): JsonlEntryData =>
  ({
    type: 'assistant', uuid: 'mix-uuid',
    message: {
      id: 'mix-msg', role: 'assistant',
      content: [
        { type: 'text', text },
        { type: 'tool_use', id: 'mix-tool', name: tool, input: {} },
      ],
    },
  }) as never

// Every type the viewer can actually draw. Anything else MUST be declared.
const RENDERABLE = new Set(['user', 'assistant', 'system', 'attachment', 'tool_group'])

// ── Scenario 1: the exact shape the user reported ───────────────────────────
console.log('Scenario 1: a bookkeeping row between tool calls must not split the burst')
{
  for (const noise of ['mode', 'ai-title', 'last-prompt', 'permission-mode', 'todo-snapshot']) {
    const entries = [
      toolCall('Edit'), toolResult(),
      toolCall('Edit'), toolResult(),
      bookkeeping(noise), // ← invisible, but it used to end the run
      toolCall('Bash'), toolResult(),
      toolCall('Bash'), toolResult(),
    ]
    const out = groupConsecutiveToolCalls(entries)
    const groups = out.filter((e) => e.type === 'tool_group')
    assert(groups.length === 1, `S1 '${noise}' between tool calls → ONE burst (got ${String(groups.length)})`)
  }
}

// ── Scenario 2: a REAL message still ends the burst ─────────────────────────
console.log('Scenario 2: a real message still splits (the filter must not over-reach)')
{
  const realUser = { type: 'user', uuid: `u${uid++}`, message: { role: 'user', content: [{ type: 'text', text: 'now do the other thing' }] } } as never as JsonlEntryData
  const entries = [toolCall('Edit'), toolResult(), realUser, toolCall('Bash'), toolResult()]
  const groups = groupConsecutiveToolCalls(entries).filter((e) => e.type === 'tool_group')
  assert(groups.length === 2, `S2 a real user message DOES split the burst (got ${String(groups.length)})`)
}

// ── Scenario 3: nothing that draws null may count as visible ────────────────
console.log('Scenario 3: every declared type is hidden')
{
  const ctx = { uuidsInRange: null, range: undefined, entries: [], toolResults: undefined, isLatestSegment: true, chainUuids: null }
  for (const t of NON_RENDERING_ENTRY_TYPES) {
    assert(!entryIsVisible(bookkeeping(t), ctx), `S3 '${t}' is not visible`)
    assert(!RENDERABLE.has(t), `S3 '${t}' is declared non-rendering AND is not renderable — the two lists agree`)
  }
}

// ── Scenario 4: the guard that would have caught `mode` ─────────────────────
// Types seen in a real 16k-entry session dump. Anything here that JsonlEntry
// can't draw must be declared — this is the check that was missing.
console.log('Scenario 4: every type from a real session is either renderable or declared')
{
  const SEEN_IN_THE_WILD = [
    'user', 'assistant', 'permission-mode', 'ai-title', 'attachment',
    'last-prompt', 'system', 'mode', 'queue-operation',
  ]
  for (const t of SEEN_IN_THE_WILD) {
    assert(RENDERABLE.has(t) || NON_RENDERING_ENTRY_TYPES.has(t),
      `S4 '${t}' renders null but is NOT declared — it will split bursts and eat the render window`)
  }
}

// ── Scenario 5: a system row that draws nothing must not count ─────────────
// `stop_hook_summary` — 928 in a real corpus, 757 in ONE session — has no
// `content`, so SystemEntry draws null. entryIsVisible had NO system branch, so
// it counted them: the same blank-chat mechanism as `mode`, keyed on subtype.
console.log('Scenario 5: content-less system rows are hidden, real ones are not')
{
  const ctx = { uuidsInRange: null, range: undefined, entries: [], toolResults: undefined, isLatestSegment: true, chainUuids: null }
  const sys = (subtype: string, content?: string): JsonlEntryData =>
    ({ type: 'system', uuid: `s${uid++}`, subtype, ...(content !== undefined ? { content } : {}) }) as never

  assert(!entryIsVisible(sys('stop_hook_summary'), ctx), 'S5 content-less stop_hook_summary is hidden')
  assert(!entryIsVisible(sys('turn_duration', 'x'), ctx), 'S5 turn_duration is hidden even with content')
  assert(!entryIsVisible(sys('local_command'), ctx), 'S5 any content-less system row is hidden')
  // …and the filter must not over-reach onto rows that DO draw:
  assert(entryIsVisible(sys('local_command', 'Reloaded skills: 79'), ctx), 'S5 a system row WITH content stays visible')
  assert(entryIsVisible(sys('compact_boundary'), ctx), 'S5 compact_boundary draws its own block — visible')
  assert(entryIsVisible(sys('api_error'), ctx), 'S5 api_error draws its own block — visible')
}

// ── Scenario 6: an attachment that draws nothing must not split a burst ────
// The burst continuer had its OWN copy of the attachment check — one that read
// `att.type` and skipped the payload guards the other three copies apply. So an
// empty queued_command (29 live) read as visible, ENDED the run, and drew
// nothing: one burst rendered as two. Now all four call isVisibleAttachment.
console.log('Scenario 6: a payload-less attachment does not split a burst')
{
  const att = (type: string, extra: Record<string, unknown> = {}): JsonlEntryData =>
    ({ type: 'attachment', uuid: `x${uid++}`, attachment: { type, ...extra } }) as never

  assert(!isVisibleAttachment(att('queued_command', { prompt: '   ' })), 'S6 a blank-prompt queued_command draws nothing')
  assert(!isVisibleAttachment(att('queued_command')), 'S6 a prompt-less queued_command draws nothing')
  assert(isVisibleAttachment(att('queued_command', { prompt: 'do it' })), 'S6 …but a real one does')
  assert(!isVisibleAttachment(att('task_reminder')), 'S6 housekeeping attachments draw nothing')
  assert(isVisibleAttachment(att('edited_text_file', { filename: 'a.ts' })), 'S6 an external edit draws')

  const entries = [
    toolCall('Edit'), toolResult(),
    att('queued_command', { prompt: '  ' }), // invisible — must not end the run
    toolCall('Bash'), toolResult(),
  ]
  const groups = groupConsecutiveToolCalls(entries).filter((e) => e.type === 'tool_group')
  assert(groups.length === 1, `S6 an invisible attachment keeps ONE burst (got ${String(groups.length)})`)

  // …and a VISIBLE attachment still splits it (the guard must not over-reach).
  const split = [toolCall('Edit'), toolResult(), att('queued_command', { prompt: 'next' }), toolCall('Bash'), toolResult()]
  const g2 = groupConsecutiveToolCalls(split).filter((e) => e.type === 'tool_group')
  assert(g2.length === 2, `S6 a real queued prompt DOES split (got ${String(g2.length)})`)
}

// ── Scenario 7: the model's prose and its tool_use share ONE message ────────
// The reported shape: an assistant message ends "...Продолжаю:" and carries the
// Bash in the SAME message. That entry isn't tool-ONLY, so it never started a
// burst — its Bash rendered inline in the prose bubble while the NEXT, tool-only
// Bash got a group of its own ("1 tool call · Bash ×1"). Whether a Bash joined
// the group came down to whether the model happened to type a sentence first.
console.log('Scenario 7: prose + tool_use in one message — the tool still joins the burst')
{
  const entries = [
    mixed('Continuing:', 'Bash'), toolResult(),
    toolCall('Bash'), toolResult(),
  ]
  const out = groupConsecutiveToolCalls(entries)
  const groups = out.filter((e) => e.type === 'tool_group')
  assert(groups.length === 1, `S7 ONE burst, not a bubble + a lone group (got ${String(groups.length)})`)
  const wrapped = (groups[0] as { _groupEntries?: JsonlEntryData[] })._groupEntries ?? []
  const tools = wrapped.filter((e) => isToolOnlyAssistantEntry(e)).length
  assert(tools === 2, `S7 BOTH Bash calls are in the burst (got ${String(tools)})`)
  // The prose must survive as its own bubble — burying it inside a collapsed
  // group would hide what the model said.
  const texts = out.filter((e) => e.type === 'assistant' && !isToolOnlyAssistantEntry(e))
  assert(texts.length === 1, `S7 the prose still renders as its own bubble (got ${String(texts.length)})`)
  const t = texts[0] as { message?: { content?: { type: string; text?: string }[] } }
  assert(
    (t.message?.content ?? []).every((b) => b.type !== 'tool_use'),
    'S7 the prose bubble no longer draws the tool inline',
  )
  assert((t as { uuid?: string }).uuid === 'mix-uuid', 'S7 the prose bubble keeps the original uuid')
}

console.log('')
if (failures === 0) console.log('✅ ALL TOOL-GROUPING CHECKS PASSED')
else { console.error(`❌ ${failures} check(s) FAILED`); process.exit(1) }
