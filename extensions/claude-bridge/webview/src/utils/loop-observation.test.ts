import { describe, expect, it } from 'vitest'
import type { JsonlEntryData } from '../types/jsonl'
import {
  analyzeTurns,
  loopSignal,
  ProjectTurnHistory,
  type CompletedTurn,
  type TurnObservation,
} from './loop-observation'

const epoch = 1_700_000_000_000
const timestamp = (ms = 0) => new Date(epoch + ms).toISOString()
const prompt = (id = 'u', ms = 0): JsonlEntryData => ({
  type: 'user',
  uuid: id,
  timestamp: timestamp(ms),
  message: { role: 'user', content: 'Please work' },
})
const call = (id: string, input: unknown = { path: 'a' }, name = 'Read'): JsonlEntryData => ({
  type: 'assistant',
  uuid: `a-${id}`,
  timestamp: timestamp(1000),
  message: { role: 'assistant', content: [{ type: 'tool_use', id, name, input }] },
})
const result = (id: string, error = true, content = 'Missing file'): JsonlEntryData => ({
  type: 'user',
  uuid: `r-${id}`,
  timestamp: timestamp(2000),
  message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: id, is_error: error, content }] },
})
const end = (durationMs = 10_000, ms = durationMs): JsonlEntryData => ({
  type: 'system',
  subtype: 'turn_duration',
  uuid: `end-${ms}`,
  timestamp: timestamp(ms),
  durationMs,
})
const base = { durationMs: 10_000, toolCalls: 2, samples: 5 }
const current = (extra: Partial<TurnObservation> = {}): TurnObservation => ({
  key: 'u',
  startedAt: epoch,
  toolCalls: 0,
  repeatedCalls: 0,
  repeatedErrors: 0,
  ...extra,
})

describe('passive turn observations', () => {
  it('compares canonical arguments while deduplicating repeated tool IDs', () => {
    const a = call('1', { path: 'a', options: { b: 2, a: 1 } })
    const b = call('2', { options: { a: 1, b: 2 }, path: 'a' })
    const analysis = analyzeTurns([
      prompt(),
      a,
      a,
      b,
      call('3', { path: 'a', options: { b: 2, a: 1 } }),
      call('4', { options: { b: 2, a: 1 }, path: 'a' }),
    ])
    expect(analysis.current).toMatchObject({ toolCalls: 4, repeatedCalls: 4 })
    expect(loopSignal(analysis.current, null, epoch + 2000, true)).toBe('repeated-calls')
  })
  it('resets an identical-call streak when the arguments or tool change', () => {
    const analysis = analyzeTurns([
      prompt(),
      call('1'),
      call('2'),
      call('3', { path: 'b' }),
      call('4', { path: 'b' }, 'Write'),
    ])
    expect(analysis.current?.repeatedCalls).toBe(1)
    expect(loopSignal(analysis.current, null, epoch + 2000, true)).toBeNull()
  })
  it('recognizes repeated errors once per result and resets on success', () => {
    const entries = [prompt(), call('1'), result('1'), result('1'), call('2'), result('2'), call('3'), result('3')]
    const analysis = analyzeTurns(entries)
    expect(analysis.current?.repeatedErrors).toBe(3)
    expect(loopSignal(analysis.current, null, epoch + 3000, true)).toBe('repeated-errors')
    expect(analyzeTurns([...entries, call('4'), result('4', false)]).current?.repeatedErrors).toBe(0)
  })
  it('does not compare unrelated tools or different errors', () => {
    const analysis = analyzeTurns([
      prompt(),
      call('1'),
      result('1'),
      call('2', {}, 'Write'),
      result('2'),
      call('3'),
      result('3', true, 'Permission denied'),
    ])
    expect(analysis.current?.repeatedErrors).toBe(1)
  })
  it('skips subagents, internal/meta prompts and provisional streaming entries', () => {
    const entries = [
      prompt(),
      { ...call('side'), isSidechain: true },
      { ...call('agent'), subagentId: 'child' },
      { ...call('stream'), __streaming: true },
      { ...prompt('meta'), isMeta: true },
      call('real'),
      result('real'),
    ]
    expect(analyzeTurns(entries).current).toMatchObject({ key: 'u', toolCalls: 1 })
    expect(analyzeTurns(entries).completed).toEqual([])
  })
  it('never conflates oversized, cyclic or excessively deep inputs', () => {
    const cycle: Record<string, unknown> = {}
    cycle.self = cycle
    for (const input of [{ text: 'x'.repeat(10000) }, cycle, Array(129).fill(1)]) {
      const analysis = analyzeTurns([prompt(), ...Array.from({ length: 5 }, (_, i) => call(`${i}`, input))])
      expect(analysis.current?.repeatedCalls).toBe(1)
      expect(loopSignal(analysis.current, null, epoch + 1000, true)).toBeNull()
    }
  })
  it('never treats trimmed display-only errors as identical complete results', () => {
    const trimmed = result('2')
    if (Array.isArray(trimmed.message!.content)) trimmed.message!.content[0]._fullLen = 400000
    const analysis = analyzeTurns([prompt(), call('1'), result('1'), call('2'), trimmed, call('3'), result('3')])
    expect(analysis.current?.repeatedErrors).toBe(1)
  })
  it('keeps CLI teammate, skill and notification records inside the current turn', () => {
    const notices = [
      'Another Claude session sent a message:\n<teammate-message teammate_id="child">done</teammate-message>',
      'Base directory for this skill: /tmp/fixture',
      '<task-notification>done</task-notification>',
      'No response requested.',
      'Permission for this tool use was denied.',
    ]
    const analysis = analyzeTurns([
      prompt(),
      call('1'),
      ...notices.map((text, i) => ({ ...prompt(`notice-${i}`), message: { role: 'user', content: text } })),
      call('2'),
    ])
    expect(analysis.current).toMatchObject({ key: 'u', toolCalls: 2 })
  })
  it('excludes raw CLI interruption markers even without a bridge annotation', () => {
    const marker = {
      ...prompt('abort', 2000),
      message: { role: 'user', content: '[Request interrupted by user for tool use]' },
    }
    expect(analyzeTurns([prompt(), call('1'), marker, end()])).toEqual({ completed: [], current: null })
  })
  it('learns CLI turn_duration and end_turn completion without counting tool results as prompts', () => {
    const analysis = analyzeTurns([
      prompt('one'),
      call('1'),
      result('1'),
      end(),
      prompt('two', 20000),
      {
        type: 'assistant',
        timestamp: timestamp(25000),
        message: { role: 'assistant', content: 'Done', stop_reason: 'end_turn' },
      },
    ])
    expect(analysis.current).toBeNull()
    expect(analysis.completed.map((t) => [t.key, t.durationMs, t.toolCalls])).toEqual([
      ['one', 10000, 1],
      ['two', 5000, 0],
    ])
  })
  it.each(['interruptedByUser', 'skippedByUser'] as const)('excludes %s turns from the baseline', (flag) => {
    const analysis = analyzeTurns([prompt(), call('1'), { type: 'user', [flag]: true }, end()])
    expect(analysis).toEqual({ completed: [], current: null })
  })
  it('drops incomplete and compacted turns without learning their duration', () => {
    const analysis = analyzeTurns([
      prompt(),
      call('1'),
      prompt('next', 20000),
      { type: 'system', subtype: 'compact_boundary' },
      end(),
    ])
    expect(analysis).toEqual({ completed: [], current: null })
  })
  it('requires a valid timestamp and stable user identity for live detection', () => {
    expect(analyzeTurns([{ ...prompt(), timestamp: 'invalid' }, call('1')]).current).toBeNull()
    expect(analyzeTurns([{ ...prompt(), uuid: undefined }, call('1')]).current).toBeNull()
  })
})

describe('project baseline and notification decision', () => {
  const turns = (n: number): CompletedTurn[] =>
    Array.from({ length: n }, (_, i) => ({
      key: `t${i}`,
      endedAt: epoch + i * 10000,
      durationMs: 10000,
      toolCalls: 2,
    }))
  it('requires five completed samples, deduplicates replay and separates projects/conversations', () => {
    const history = new ProjectTurnHistory()
    expect(history.observe('server/project-a', 'conversation', turns(4))).toBeNull()
    expect(history.observe('server/project-a', 'conversation', turns(5))).toEqual(base)
    expect(history.observe('server/project-a', 'conversation', turns(5))).toEqual(base)
    expect(history.observe('server/project-b', 'conversation', turns(4))).toBeNull()
    expect(history.observe('server/project-a', 'another-conversation', turns(5))?.samples).toBe(10)
  })
  it('uses the median of the latest forty turns, so one long turn does not set the norm', () => {
    const history = new ProjectTurnHistory()
    const values = turns(45)
    values[44].durationMs = 10_000_000
    expect(history.observe('p', 'c', values)).toEqual({ ...base, samples: 40 })
  })
  it('bounds project history to thirty-two recently observed projects', () => {
    const history = new ProjectTurnHistory()
    for (let i = 0; i < 33; i++) history.observe(`p${i}`, 'c', turns(5))
    expect(history.observe('p0', 'c', [])).toBeNull()
    expect(history.observe('p32', 'c', [])).toEqual(base)
  })
  it('signals strictly above 2.5× the project norm, including turns without tools', () => {
    expect(loopSignal(current(), base, epoch + 25000, true)).toBeNull()
    expect(loopSignal(current(), base, epoch + 25001, true)).toBe('long-turn')
    expect(loopSignal(current({ toolCalls: 5 }), base, epoch + 1000, true)).toBeNull()
    expect(loopSignal(current({ toolCalls: 6 }), base, epoch + 1000, true)).toBe('many-calls')
  })
  it('requires live working state and never alerts on completed/historical turns', () => {
    expect(loopSignal(current({ repeatedCalls: 10 }), base, epoch + 100000, false)).toBeNull()
    expect(loopSignal(null, base, epoch + 100000, true)).toBeNull()
    expect(loopSignal(current({ repeatedCalls: 10 }), base, epoch - 1, true)).toBeNull()
    expect(loopSignal(current(), null, epoch + 100000, true)).toBeNull()
    expect(loopSignal(current(), { ...base, samples: 4 }, epoch + 100000, true)).toBeNull()
  })
  it('does not mutate any incoming JSONL or tool definition', () => {
    const entries = [prompt(), call('1'), result('1'), end()]
    const original = JSON.stringify(entries)
    analyzeTurns(entries)
    expect(JSON.stringify(entries)).toBe(original)
  })
})
