import { describe, expect, it } from 'vitest'
import type { AgentEvent } from '../../shared/agent-events'
import { CodexHost } from './codex-host'
import { CodexAgentSession } from './codex-session'
import { bridgeMcpThreadConfig } from './config-toml'
import { FakeCodexProcess, flush } from './fixtures/fake-process'

const thread = (id: string) => ({ thread: { id, model: 'gpt-5.5', cwd: '/w', path: null }, model: 'gpt-5.5' })

function setup(extra: Record<string, (p: any) => unknown> = {}) {
  let turns = 0
  const proc = new FakeCodexProcess({
    'thread/start': () => thread('t1'),
    'thread/resume': (p) => thread(p.threadId),
    'turn/start': () => ({ turn: { id: `u${++turns}`, status: 'inProgress', error: null } }),
    'turn/steer': (p) => ({ turnId: p.expectedTurnId }),
    'turn/interrupt': () => ({}),
    'thread/unsubscribe': () => ({ status: 'unsubscribed' }),
    ...extra,
  })
  const host = new CodexHost(proc)
  return { proc, host }
}

async function started(
  extra?: Record<string, (p: any) => unknown>,
  options: Partial<Parameters<typeof CodexAgentSession.start>[0]> = {},
) {
  const { proc, host } = setup(extra)
  const session = await CodexAgentSession.start({ host, cwd: '/w', model: 'gpt-5.5', ...options })
  const events: AgentEvent[] = []
  session.events.subscribe((e) => events.push(e))
  return { proc, host, session, events }
}

describe('CodexAgentSession', () => {
  it('starts a locked-down thread carrying the session MCP endpoint', async () => {
    const config = bridgeMcpThreadConfig({ url: 'http://127.0.0.1:1/mcp/s', token: 'tok' })
    const { proc, session } = await started(undefined, { threadConfig: config, developerInstructions: 'be Bridge' })
    expect(proc.requests('thread/start')).toEqual([
      {
        model: 'gpt-5.5',
        cwd: '/w',
        approvalPolicy: 'never',
        sandbox: 'read-only',
        config,
        developerInstructions: 'be Bridge',
      },
    ])
    expect(session.conversationId()).toBe('t1')
    expect(session.capabilities).toEqual({ nativeSteer: true, liveModelSwitch: true, normalizedTranscript: true })
  })

  it('resumes by thread id and re-sends the thread config', async () => {
    const config = bridgeMcpThreadConfig({ url: 'http://127.0.0.1:1/mcp/s2', token: 'tok2' })
    const { proc, session } = await started(undefined, { resumeThreadId: 'old', threadConfig: config })
    expect(proc.requests('thread/resume')).toEqual([
      expect.objectContaining({ threadId: 'old', config, excludeTurns: true, approvalPolicy: 'never' }),
    ])
    expect(session.conversationId()).toBe('old')
  })

  it('emits session.started, then the thread notifications it owns', async () => {
    const { proc, events } = await started()
    proc.notify('turn/started', { threadId: 'other', turn: { id: 'x' } })
    proc.notify('turn/started', { threadId: 't1', turn: { id: 'u1' } }, 7)
    await flush()
    expect(events).toEqual([
      expect.objectContaining({ kind: 'session.started', engine: 'codex', model: 'gpt-5.5', resumed: false }),
      { kind: 'turn.started', threadId: 't1', turnId: 'u1', at: 7 },
    ])
  })

  it('sends a turn with submission id, pending model and effort; steers while it runs', async () => {
    const { proc, session } = await started(undefined, { effort: 'high' })
    expect(await session.setModel('gpt-6-sol')).toBe('next-turn')
    await session.send('hello', 'sub-1')
    expect(proc.requests('turn/start')).toEqual([
      {
        threadId: 't1',
        input: [{ type: 'text', text: 'hello', text_elements: [] }],
        clientUserMessageId: 'sub-1',
        model: 'gpt-6-sol',
        effort: 'high',
      },
    ])
    expect(session.running).toBe(true)
    await session.send('more', 'sub-2')
    expect(proc.requests('turn/steer')).toEqual([
      {
        threadId: 't1',
        input: [{ type: 'text', text: 'more', text_elements: [] }],
        expectedTurnId: 'u1',
        clientUserMessageId: 'sub-2',
      },
    ])
    proc.notify('turn/completed', { threadId: 't1', turn: { id: 'u1', status: 'completed', error: null } })
    await flush()
    expect(session.running).toBe(false)
    await session.send('next')
    expect(proc.requests('turn/start')[1]).not.toHaveProperty('model') // the switch applied once
  })

  it('starts a new turn when a steer loses the race with turn completion', async () => {
    let proc!: FakeCodexProcess
    const ctx = await started({
      'turn/steer': () => {
        proc.notify('turn/completed', { threadId: 't1', turn: { id: 'u1', status: 'completed', error: null } })
        throw new Error('no active turn')
      },
    })
    proc = ctx.proc
    await ctx.session.send('first')
    await ctx.session.steer('late')
    expect(proc.requests('turn/start').map((p) => p.input[0].text)).toEqual(['first', 'late'])
  })

  it('does not adopt a turn that already completed before turn/start answered', async () => {
    let proc!: FakeCodexProcess
    const ctx = await started({
      'turn/start': () => {
        proc.notify('turn/started', { threadId: 't1', turn: { id: 'u9' } })
        proc.notify('turn/completed', { threadId: 't1', turn: { id: 'u9', status: 'completed', error: null } })
        return { turn: { id: 'u9', status: 'inProgress', error: null } }
      },
    })
    proc = ctx.proc
    await ctx.session.send('fast')
    await flush()
    expect(ctx.session.running).toBe(false)
    await expect(ctx.session.waitForIdle(10)).resolves.toBeUndefined()
  })

  it('interrupts only a running turn', async () => {
    const { proc, session } = await started()
    await session.interrupt()
    expect(proc.requests('turn/interrupt')).toEqual([])
    await session.send('go')
    await session.interrupt()
    expect(proc.requests('turn/interrupt')).toEqual([{ threadId: 't1', turnId: 'u1' }])
  })

  it('waitForIdle resolves on completion and times out otherwise', async () => {
    const { proc, session } = await started()
    await session.send('go')
    await expect(session.waitForIdle(10)).rejects.toThrow('still running')
    const idle = session.waitForIdle(1_000)
    proc.notify('turn/completed', { threadId: 't1', turn: { id: 'u1', status: 'completed', error: null } })
    await expect(idle).resolves.toBeUndefined()
  })

  it('stop interrupts, unsubscribes, detaches and ends the stream once', async () => {
    const { proc, host, session, events } = await started()
    await session.send('go')
    await session.stop('tab_closed')
    await session.stop()
    expect(proc.requests('turn/interrupt')).toEqual([{ threadId: 't1', turnId: 'u1' }])
    expect(proc.requests('thread/unsubscribe')).toEqual([{ threadId: 't1' }])
    expect(host.threadCount).toBe(0)
    expect(events.filter((e) => e.kind === 'session.exited')).toEqual([
      expect.objectContaining({ reason: 'tab_closed', code: null }),
    ])
    expect(session.events.closed).toBe(true)
    await expect(session.send('after')).rejects.toThrow('not running')
  })

  it('reports an app-server exit to the session', async () => {
    const { proc, session, events } = await started()
    await session.send('go')
    const idle = session.waitForIdle(1_000)
    proc.crash({ code: 101, signal: null })
    await expect(idle).resolves.toBeUndefined()
    expect(events.at(-1)).toMatchObject({ kind: 'session.exited', code: 101, reason: 'codex app-server exited' })
    await expect(session.send('x')).rejects.toThrow('not running')
  })

  it('propagates a failed thread start', async () => {
    const { host } = setup({
      'thread/start': () => {
        throw new Error('mcp bridge required but failed')
      },
    })
    await expect(CodexAgentSession.start({ host, cwd: '/w' })).rejects.toThrow('mcp bridge required')
    expect(host.threadCount).toBe(0)
  })
})
