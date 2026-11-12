import { describe, expect, it } from 'vitest'
import type { AgentEvent } from '../../shared/agent-events'
import { GATE_TRACE, loadTrace, notifications, threadByPrompt } from './fixtures/load'
import { CodexEventNormalizer } from './normalize'

const trace = loadTrace(GATE_TRACE)
const threads = threadByPrompt(trace)

function eventsFor(threadId: string): AgentEvent[] {
  const n = new CodexEventNormalizer(() => 0)
  return notifications(trace)
    .filter((f) => f.params?.threadId === threadId)
    .flatMap((f) => n.normalize(f.method, f.params, f.emittedAtMs))
}

const at = 1_000
const n = () => new CodexEventNormalizer(() => at)
const item = (completed: boolean, item: Record<string, unknown>) =>
  [
    completed ? 'item/completed' : 'item/started',
    { threadId: 't', turnId: 'u', item, [completed ? 'completedAtMs' : 'startedAtMs']: 5 },
  ] as const

describe('CodexEventNormalizer on the recorded 0.160.1 gate trace', () => {
  it('projects a direct-tool turn: refusals stay invisible, MCP call, reasoning, streamed text, usage', () => {
    const events = eventsFor(threads['probe-a']!)
    const kinds = events.map((e) => e.kind)
    expect(kinds[0]).toBe('turn.started')
    expect(kinds.at(-1)).toBe('turn.completed')
    expect(events.find((e) => e.kind === 'message.user')).toMatchObject({ text: 'probe-a' })
    // exec_command/shell/apply_patch were refused inside codex: no tool item reached Bridge.
    const tools = events.filter((e) => e.kind === 'tool.started' || e.kind === 'tool.completed')
    expect(tools.map((e) => e.kind)).toEqual(['tool.started', 'tool.completed'])
    expect(tools[1]).toMatchObject({
      tool: { source: 'mcp', server: 'bridge', name: 'Read' },
      input: { path: 'a.txt' },
      status: 'completed',
      output: [{ type: 'text', text: 'MCP-OK Read' }],
    })
    expect(events.find((e) => e.kind === 'reasoning')).toMatchObject({ summary: ['Thinking about it'] })
    expect(events.filter((e) => e.kind === 'reasoning.delta').map((e: any) => e.delta)).toEqual(['Thinking about it'])
    const deltas = events.filter((e) => e.kind === 'message.assistant.delta')
    expect(deltas.map((e: any) => e.delta).join('')).toBe('all done')
    expect(events.find((e) => e.kind === 'message.assistant')).toMatchObject({ text: 'all done', id: deltas[0]!.id })
    const usage = events.filter((e) => e.kind === 'usage')
    expect(usage.length).toBeGreaterThan(0)
    expect(usage.at(-1)).toMatchObject({ last: { inputTokens: 10, outputTokens: 2, totalTokens: 12 } })
    expect(events.find((e) => e.kind === 'turn.completed')).toMatchObject({ status: 'completed' })
    expect(events.every((e) => e.threadId === threads['probe-a'] && typeof e.at === 'number' && e.at > 0)).toBe(true)
  })

  it('surfaces MCP calls made from code-mode JavaScript as ordinary tool events', () => {
    const events = eventsFor(threads['probe-c']!)
    expect(events.filter((e) => e.kind === 'tool.completed')).toEqual([
      expect.objectContaining({ tool: { source: 'mcp', server: 'bridge', name: 'Read' }, input: { path: 'c.txt' } }),
    ])
    expect(events.some((e) => e.kind === 'item.unsupported')).toBe(false)
  })

  it('keeps interleaved parallel threads apart by thread id', () => {
    const b = eventsFor(threads['probe-b']!)
    expect(b.filter((e) => e.kind === 'tool.completed').map((e: any) => e.tool.name)).toEqual(['Bash'])
    expect(b.find((e) => e.kind === 'message.assistant')).toMatchObject({ text: 'b done' })
  })

  it('ignores account- and server-level notifications', () => {
    const quiet = n()
    for (const method of [
      'account/rateLimits/updated',
      'remoteControl/status/changed',
      'mcpServer/startupStatus/updated',
      'thread/status/changed',
      'thread/started',
    ]) {
      expect(quiet.normalize(method, { threadId: 't' })).toEqual([])
    }
  })
})

describe('CodexEventNormalizer edge cases', () => {
  it('maps failed and interrupted turns with their error', () => {
    const failed = n().normalize('turn/completed', {
      threadId: 't',
      turn: {
        id: 'u',
        status: 'failed',
        error: { message: 'usage limit', additionalDetails: 'try later', codexErrorInfo: null },
        durationMs: 9,
      },
    })
    expect(failed).toEqual([
      {
        threadId: 't',
        at,
        kind: 'turn.completed',
        turnId: 'u',
        status: 'failed',
        error: { message: 'usage limit', details: 'try later' },
        durationMs: 9,
      },
    ])
    expect(
      n().normalize('turn/completed', { threadId: 't', turn: { id: 'u', status: 'interrupted', error: null } }),
    ).toEqual([expect.objectContaining({ status: 'interrupted' })])
  })

  it('uses the frame emittedAtMs, else item lifecycle timestamps, else the clock', () => {
    expect(n().normalize('turn/started', { threadId: 't', turn: { id: 'u' } }, 42)[0]!.at).toBe(42)
    expect(n().normalize(...item(true, { type: 'agentMessage', id: 'm', text: 'x' }))[0]!.at).toBe(5)
    expect(n().normalize('turn/started', { threadId: 't', turn: { id: 'u' } })[0]!.at).toBe(at)
  })

  it('maps compaction from the item and ignores the deprecated thread/compacted twin', () => {
    expect(n().normalize(...item(true, { type: 'contextCompaction', id: 'c1' }))).toEqual([
      expect.objectContaining({ kind: 'context.compaction', id: 'c1', turnId: 'u' }),
    ])
    expect(n().normalize(...item(false, { type: 'contextCompaction', id: 'c1' }))).toEqual([])
    expect(n().normalize('thread/compacted', { threadId: 't', turnId: 'u' })).toEqual([])
  })

  it('maps errors, warnings and model reroutes', () => {
    expect(
      n().normalize('error', { threadId: 't', turnId: 'u', willRetry: true, error: { message: 'stream lost' } }),
    ).toEqual([
      expect.objectContaining({ kind: 'notice', level: 'error', message: 'stream lost', willRetry: true, turnId: 'u' }),
    ])
    expect(n().normalize('warning', { threadId: null, message: 'heads up' })).toEqual([
      expect.objectContaining({ kind: 'notice', level: 'warning', message: 'heads up' }),
    ])
    expect(
      n().normalize('model/rerouted', { threadId: 't', turnId: 'u', fromModel: 'a', toModel: 'b', reason: 'capacity' }),
    ).toEqual([expect.objectContaining({ kind: 'model.rerouted', from: 'a', to: 'b', reason: 'capacity' })])
  })

  it('maps MCP failures, images and dynamic tools', () => {
    const [failed] = n().normalize(
      ...item(true, {
        type: 'mcpToolCall',
        id: 'x',
        server: 'bridge',
        tool: 'Bash',
        status: 'failed',
        arguments: {},
        result: null,
        error: { message: 'denied' },
        durationMs: 3,
      }),
    )
    expect(failed).toMatchObject({
      kind: 'tool.completed',
      status: 'failed',
      error: 'denied',
      output: [],
      durationMs: 3,
    })
    const [image] = n().normalize(
      ...item(true, {
        type: 'mcpToolCall',
        id: 'y',
        server: 'bridge',
        tool: 'Read',
        status: 'completed',
        arguments: {},
        result: {
          content: [
            { type: 'image', data: 'AAA', mimeType: 'image/jpeg' },
            { type: 'resource', uri: 'x' },
          ],
        },
        error: null,
      }),
    )
    expect(image).toMatchObject({
      output: [
        { type: 'image', url: 'data:image/jpeg;base64,AAA' },
        { type: 'json', value: { type: 'resource', uri: 'x' } },
      ],
    })
    const [dyn] = n().normalize(
      ...item(true, {
        type: 'dynamicToolCall',
        id: 'z',
        namespace: null,
        tool: 'probe',
        arguments: { a: 1 },
        status: 'completed',
        contentItems: [{ type: 'inputText', text: 'ok' }],
        success: false,
      }),
    )
    expect(dyn).toMatchObject({
      tool: { source: 'dynamic', name: 'probe' },
      status: 'failed',
      output: [{ type: 'text', text: 'ok' }],
    })
  })

  it('keeps a built-in command visible (declined) if one ever appears', () => {
    const [started] = n().normalize(
      ...item(false, { type: 'commandExecution', id: 'k', command: 'rm -rf /', status: 'inProgress' }),
    )
    expect(started).toMatchObject({
      kind: 'tool.started',
      tool: { source: 'builtin', name: 'command' },
      input: { command: 'rm -rf /' },
    })
    const [done] = n().normalize(
      ...item(true, {
        type: 'commandExecution',
        id: 'k',
        command: 'rm -rf /',
        status: 'declined',
        aggregatedOutput: null,
      }),
    )
    expect(done).toMatchObject({ kind: 'tool.completed', status: 'declined', output: [] })
  })

  it('maps provider-side web search as a built-in tool', () => {
    const search = {
      type: 'webSearch',
      id: 'w',
      query: 'kamin',
      action: { type: 'search', query: 'kamin', queries: null },
    }
    expect(n().normalize(...item(false, search))).toEqual([
      expect.objectContaining({
        kind: 'tool.started',
        tool: { source: 'builtin', name: 'web_search' },
        input: { query: 'kamin', action: search.action },
      }),
    ])
    expect(n().normalize(...item(true, { ...search, results: [{ url: 'u' }] }))).toEqual([
      expect.objectContaining({
        kind: 'tool.completed',
        status: 'completed',
        output: [{ type: 'json', value: { url: 'u' } }],
      }),
    ])
  })

  it('reports unknown item types instead of dropping them', () => {
    expect(n().normalize(...item(true, { type: 'plan', id: 'p', text: 'x' }))).toEqual([
      expect.objectContaining({ kind: 'item.unsupported', itemType: 'plan', id: 'p' }),
    ])
    expect(n().normalize(...item(false, { type: 'plan', id: 'p' }))).toEqual([])
  })
})
