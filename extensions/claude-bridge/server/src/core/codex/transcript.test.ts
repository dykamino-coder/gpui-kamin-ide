import { describe, expect, it } from 'vitest'
import type { AgentEvent } from '../../shared/agent-events'
import { GATE_TRACE, loadTrace, notifications, threadByPrompt } from './fixtures/load'
import { CodexEventNormalizer } from './normalize'
import { CodexTranscriptProjector, type CodexTranscriptEntry } from './transcript'

const trace = loadTrace(GATE_TRACE)
const threads = threadByPrompt(trace)

function transcriptFor(threadId: string): CodexTranscriptEntry[] {
  const normalizer = new CodexEventNormalizer(() => 0)
  const projector = new CodexTranscriptProjector('/work/ws')
  projector.project({ kind: 'session.started', threadId, at: 0, engine: 'codex', model: 'gpt-5.5', resumed: false })
  return notifications(trace)
    .filter((f) => f.params?.threadId === threadId)
    .flatMap((f) => normalizer.normalize(f.method, f.params, f.emittedAtMs))
    .flatMap((e) => projector.project(e))
}

describe('CodexTranscriptProjector', () => {
  it('writes the recorded turn as a linked Bridge transcript marked engine=codex', () => {
    const entries = transcriptFor(threads['probe-a']!)
    expect(entries.map((e) => [e.type, (e.message?.content as any)?.[0]?.type ?? 'text'])).toEqual([
      ['user', 'text'],
      ['assistant', 'tool_use'],
      ['user', 'tool_result'],
      ['assistant', 'thinking'],
      ['assistant', 'text'],
    ])
    expect(
      entries.every((e) => e.engine === 'codex' && e.sessionId === threads['probe-a'] && e.cwd === '/work/ws'),
    ).toBe(true)
    // One chain: each entry points at the previous one.
    entries.forEach((e, i) => expect(e.parentUuid).toBe(i === 0 ? null : entries[i - 1]!.uuid))
    expect(new Set(entries.map((e) => e.uuid)).size).toBe(entries.length)
    const [, toolUse, toolResult, , text] = entries
    const use = (toolUse!.message!.content as any[])[0]
    expect(use).toMatchObject({ type: 'tool_use', name: 'mcp__bridge__Read', input: { path: 'a.txt' } })
    expect((toolResult!.message!.content as any[])[0]).toEqual({
      type: 'tool_result',
      tool_use_id: use.id,
      content: [{ type: 'text', text: 'MCP-OK Read' }],
    })
    expect(text!.message).toMatchObject({
      role: 'assistant',
      model: 'gpt-5.5',
      content: [{ type: 'text', text: 'all done' }],
    })
    expect(entries[0]!.message).toEqual({ role: 'user', content: 'probe-a' })
    expect(entries.every((e) => !Number.isNaN(Date.parse(e.timestamp!)))).toBe(true)
  })

  const base = { threadId: 't', at: 0 }
  const project = (...events: AgentEvent[]) => {
    const p = new CodexTranscriptProjector()
    return events.flatMap((e) => p.project(e))
  }

  it('marks failures, interrupts, compaction and reroutes as system entries', () => {
    const entries = project(
      { ...base, kind: 'model.rerouted', turnId: 'u', from: 'a', to: 'b', reason: 'capacity' },
      { ...base, kind: 'context.compaction', turnId: 'u', id: 'c1' },
      { ...base, kind: 'turn.completed', turnId: 'u', status: 'interrupted' },
      { ...base, kind: 'turn.completed', turnId: 'v', status: 'failed', error: { message: 'usage limit reached' } },
      { ...base, kind: 'turn.completed', turnId: 'w', status: 'completed' },
      { ...base, kind: 'notice', level: 'error', message: 'retrying', willRetry: true },
      { ...base, kind: 'item.unsupported', turnId: 'w', itemType: 'plan' },
    )
    expect(entries.map((e) => [e.type, e.subtype, e.level, e.content, e.interruptedByUser])).toEqual([
      ['system', undefined, 'info', 'Model rerouted from a to b (capacity)', undefined],
      ['system', 'compact_boundary', 'info', 'Conversation compacted', undefined],
      ['system', undefined, 'info', 'Interrupted by user', true],
      ['system', undefined, 'error', 'usage limit reached', undefined],
      ['system', undefined, 'warning', 'Unsupported codex item: plan', undefined],
    ])
    expect(entries[1]!.compactMetadata).toEqual({ trigger: 'auto' })
  })

  it('renders web search with the WebSearch card and failed tools with their error', () => {
    const tool = { source: 'builtin', name: 'web_search' } as const
    const [use] = project({
      ...base,
      kind: 'tool.completed',
      id: 's',
      turnId: 'u',
      tool,
      input: { query: 'q' },
      status: 'completed',
      output: [],
    })
    expect((use!.message!.content as any[])[0]).toMatchObject({ name: 'WebSearch', input: { query: 'q' } })
    const [, result] = project({
      ...base,
      kind: 'tool.completed',
      id: 'x',
      turnId: 'u',
      tool: { source: 'mcp', server: 'bridge_interactive', name: 'AskUserQuestion' },
      input: 'raw',
      status: 'failed',
      output: [],
      error: 'cancelled',
    })
    expect((result!.message!.content as any[])[0]).toMatchObject({
      content: [{ type: 'text', text: 'Error: cancelled' }],
    })
  })

  it('skips empty reasoning and transport-only events', () => {
    expect(
      project(
        { ...base, kind: 'reasoning', id: 'r', turnId: 'u', summary: [], content: [] },
        { ...base, kind: 'message.assistant.delta', id: 'm', turnId: 'u', delta: 'x' },
        { ...base, kind: 'usage', turnId: 'u', total: {} as any, last: {} as any, contextWindow: null },
        { ...base, kind: 'turn.started', turnId: 'u' },
      ),
    ).toEqual([])
  })
})
