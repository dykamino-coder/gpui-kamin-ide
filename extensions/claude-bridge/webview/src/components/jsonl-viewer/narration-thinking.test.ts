// INC-2026-0063: assistant narration shown in the Bridge Console was missing
// from the chat. Synthetic fixture (no real session text) with the exact row
// shape CLI 2.1.x persisted: one model turn = thinking(empty, signature only)
// + thinking(narration, `thinkingDisplay: "updates"`) + tool_use, each its own
// JSONL row under one message.id, then the user tool_result row.
import { describe, expect, it } from 'vitest'
import type { ContentBlock, JsonlEntryData } from '../../types/jsonl'
import { getRenderModel } from './derived-cache'
import { groupConsecutiveToolCalls, isToolOnlyAssistantEntry, surfaceNarrationThinking } from './utils'

let seq = 0
function turn(msgId: string, narration: string, tool: string, display: string | undefined): JsonlEntryData[] {
  const base = (blocks: ContentBlock[], extra: Record<string, unknown> = {}): JsonlEntryData =>
    ({
      type: 'assistant', uuid: `u-${seq++}`, ...extra,
      message: { id: msgId, role: 'assistant', stop_reason: 'tool_use', content: blocks },
    }) as JsonlEntryData
  const shown = display ? { thinkingDisplay: display } : {}
  return [
    // apiBlockIndex 0: hidden reasoning, text stripped (signature removed by the server projection)
    base([{ type: 'thinking', thinking: '' }], shown),
    // apiBlockIndex 1: the narration the TUI prints
    base([{ type: 'thinking', thinking: narration }], shown),
    base([{ type: 'tool_use', id: `${msgId}-t`, name: tool, input: {} }]),
    {
      type: 'user', uuid: `u-${seq++}`,
      message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: `${msgId}-t`, content: 'ok' }] },
    } as JsonlEntryData,
  ]
}

const NARRATION_A = 'Synthetic narration A: fixing the build error first.'
const NARRATION_B = 'Synthetic narration B: build ok, now the round button.'

function textsOf(entries: JsonlEntryData[]): string[] {
  const out: string[] = []
  for (const e of entries) {
    if (e.type !== 'assistant' || !Array.isArray(e.message?.content)) continue
    for (const b of e.message!.content as ContentBlock[]) {
      if (b.type === 'text' && b.text) out.push(b.text)
    }
  }
  return out
}

describe('INC-2026-0063 narration thinking between tool calls', () => {
  const fixture = (): JsonlEntryData[] => [
    { type: 'user', uuid: 'u-first', message: { role: 'user', content: 'synthetic request' } } as JsonlEntryData,
    ...turn('msg_syn_a', NARRATION_A, 'Read', 'updates'),
    ...turn('msg_syn_b', NARRATION_B, 'Bash', 'updates'),
  ]

  it('renders every narration as a standalone prose bubble outside the tool groups', () => {
    const model = getRenderModel('inc-0063-tab', fixture(), [], 0, 1)
    const standalone = model.merged.filter((e) => e.type === 'assistant')
    expect(textsOf(standalone)).toEqual([NARRATION_A, NARRATION_B])
    // The tools are still grouped — one burst per turn, tool_use only.
    const groups = model.merged.filter((e) => e.type === 'tool_group')
    expect(groups).toHaveLength(2)
    for (const g of groups) {
      for (const inner of g._groupEntries ?? []) {
        if (inner.type === 'assistant') expect(isToolOnlyAssistantEntry(inner)).toBe(true)
      }
    }
    // Order mirrors the console: narration, then its tool burst.
    expect(model.merged.map((e) => e.type)).toEqual(['user', 'assistant', 'tool_group', 'assistant', 'tool_group'])
  })

  it('never mutates committed store entries', () => {
    const entries = fixture()
    const snapshot = JSON.stringify(entries)
    getRenderModel('inc-0063-tab-2', entries, [], 0, 1)
    expect(JSON.stringify(entries)).toBe(snapshot)
    const plain = [{ type: 'assistant', uuid: 'x', message: { role: 'assistant', content: [{ type: 'text', text: 'hi' }] } } as JsonlEntryData]
    expect(surfaceNarrationThinking(plain)).toBe(plain)
  })

  it('keeps non-narration thinking visible (collapsed) instead of hiding it in the group', () => {
    const merged = [
      ...turn('msg_syn_c', 'Synthetic legacy thinking text.', 'Grep', undefined),
    ]
    const model = getRenderModel('inc-0063-tab-3', merged, [], 0, 1)
    const prose = model.merged.filter((e) => e.type === 'assistant')
    expect(prose).toHaveLength(1)
    const blocks = prose[0]!.message!.content as ContentBlock[]
    expect(blocks.some((b) => b.type === 'thinking' && b.thinking === 'Synthetic legacy thinking text.')).toBe(true)
    expect(blocks.some((b) => b.type === 'tool_use')).toBe(false)
    expect(model.merged.filter((e) => e.type === 'tool_group')).toHaveLength(1)
  })

  it('still folds empty-thinking + tool_use entries into one burst', () => {
    const entries: JsonlEntryData[] = [
      { type: 'assistant', uuid: 'e1', message: { id: 'm1', role: 'assistant', content: [{ type: 'thinking', thinking: '' }, { type: 'tool_use', id: 't1', name: 'Read', input: {} }] } },
      { type: 'user', uuid: 'e2', message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: 't1', content: 'ok' }] } },
      { type: 'assistant', uuid: 'e3', message: { id: 'm2', role: 'assistant', content: [{ type: 'tool_use', id: 't2', name: 'Bash', input: {} }] } },
    ] as JsonlEntryData[]
    const out = groupConsecutiveToolCalls(entries)
    expect(out.map((e) => e.type)).toEqual(['tool_group'])
  })
})
