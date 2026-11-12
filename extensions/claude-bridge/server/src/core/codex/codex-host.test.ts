import { afterEach, describe, expect, it, vi } from 'vitest'
import { GATE_TRACE, loadTrace, threadByPrompt } from './fixtures/load'
import { FakeCodexProcess, flush } from './fixtures/fake-process'
import { answerServerRequest, CodexHost, CodexHostRegistry, type ThreadSink } from './codex-host'
import { JsonRpcError, METHOD_NOT_FOUND } from './jsonrpc'

function sink() {
  const seen: Array<{ method: string; params: any; at: unknown }> = []
  const exits: unknown[] = []
  const s: ThreadSink = {
    onNotification: (method, params, at) => seen.push({ method, params, at }),
    onExit: (exit) => exits.push(exit),
  }
  return { s, seen, exits }
}

afterEach(() => vi.useRealTimers())

describe('CodexHost routing', () => {
  it('routes the interleaved recorded trace to one sink per thread', async () => {
    const proc = new FakeCodexProcess()
    const host = new CodexHost(proc)
    const threads = threadByPrompt(loadTrace(GATE_TRACE))
    const sinks = Object.fromEntries(Object.entries(threads).map(([k, id]) => [k, { id, ...sink() }]))
    for (const { id, s } of Object.values(sinks)) host.attach(id, s)
    const account = vi.fn()
    host.onHostNotification(account)
    const frames = loadTrace(GATE_TRACE)
      .filter((f) => f.direction === 'in' && f.message.method && f.message.id === undefined)
      .map((f) => f.message)
    proc.send(...frames)
    await flush()
    for (const { id, seen } of Object.values(sinks)) {
      expect(seen.length).toBeGreaterThan(5)
      expect(seen.every((f) => (f.params.threadId ?? f.params.thread?.id) === id)).toBe(true)
      expect(seen.filter((f) => f.method === 'turn/completed')).toHaveLength(1)
    }
    const threadless = frames.filter((f) => f.params?.threadId === undefined && f.method !== 'thread/started')
    expect(account).toHaveBeenCalledTimes(threadless.length)
    expect(account.mock.calls.map((c) => c[0])).toContain('account/rateLimits/updated')
  })

  it('buffers frames that arrive before attach and replays them in order', async () => {
    const proc = new FakeCodexProcess()
    const host = new CodexHost(proc)
    // Response and first notifications in one chunk, as codex writes them.
    proc.notify('thread/started', { thread: { id: 't1' } }, 1)
    proc.notify('turn/started', { threadId: 't1', turn: { id: 'u' } }, 2)
    await flush()
    const { s, seen } = sink()
    host.attach('t1', s)
    expect(seen.map((f) => [f.method, f.at])).toEqual([
      ['thread/started', 1],
      ['turn/started', 2],
    ])
    proc.notify('turn/completed', { threadId: 't1', turn: { id: 'u' } })
    await flush()
    expect(seen).toHaveLength(3)
  })

  it('drops frames of detached threads and bounds the early buffer', async () => {
    const proc = new FakeCodexProcess()
    const host = new CodexHost(proc, { maxBufferedPerThread: 2, maxBufferedThreads: 1, idleCloseMs: 60_000 })
    const a = sink()
    host.attach('a', a.s)()
    proc.notify('turn/started', { threadId: 'a', turn: { id: 'u' } })
    for (let i = 0; i < 5; i++) proc.notify('warning', { threadId: 'x', message: String(i) })
    proc.notify('warning', { threadId: 'y', message: 'evicts x' })
    await flush()
    expect(a.seen).toEqual([])
    const x = sink()
    host.attach('x', x.s)
    expect(x.seen).toEqual([]) // evicted by y (one buffered thread max)
    const y = sink()
    host.attach('y', y.s)
    expect(y.seen.map((f) => f.params.message)).toEqual(['evicts x'])
    const late = sink()
    host.attach('a', late.s) // re-attaching a retired thread (resume) works again
    proc.notify('turn/started', { threadId: 'a', turn: { id: 'v' } })
    await flush()
    expect(late.seen).toHaveLength(1)
  })

  it('answers codex requests with the decline policy', async () => {
    const proc = new FakeCodexProcess()
    new CodexHost(proc)
    proc.send(
      { id: 1, method: 'item/commandExecution/requestApproval', params: { threadId: 't' } },
      { id: 2, method: 'applyPatchApproval', params: {} },
      { id: 3, method: 'mcpServer/elicitation/request', params: {} },
      { id: 4, method: 'item/tool/call', params: {} },
      { id: 5, method: 'account/chatgptAuthTokens/refresh', params: {} },
    )
    await flush()
    await flush()
    const byId = Object.fromEntries(proc.replies().map((r) => [r.id, r]))
    expect(byId[1]).toEqual({ id: 1, result: { decision: 'decline' } })
    expect((byId[2] as any).result.decision.denied.rejection).toMatch(/Bridge/)
    expect(byId[3]).toEqual({ id: 3, result: { action: 'decline', content: null, _meta: null } })
    expect((byId[4] as any).result).toMatchObject({ success: false })
    expect(byId[5]).toMatchObject({ error: { code: METHOD_NOT_FOUND } })
  })

  it('notifies every sink when the process exits and refuses new threads', () => {
    const proc = new FakeCodexProcess()
    const host = new CodexHost(proc)
    const a = sink()
    const b = sink()
    host.attach('a', a.s)
    host.attach('b', b.s)
    const closed = vi.fn()
    host.onClose(closed)
    proc.crash({ code: 3, signal: null })
    expect(a.exits).toEqual([{ code: 3, signal: null }])
    expect(b.exits).toEqual([{ code: 3, signal: null }])
    expect(closed).toHaveBeenCalledOnce()
    expect(host.alive).toBe(false)
    expect(() => host.attach('c', sink().s)).toThrow('closed')
  })

  it('closes the process after the last thread detached and the idle grace passed', () => {
    vi.useFakeTimers()
    const proc = new FakeCodexProcess()
    const host = new CodexHost(proc, { idleCloseMs: 1_000 })
    const detach = host.attach('a', sink().s)
    detach()
    vi.advanceTimersByTime(500)
    host.attach('b', sink().s)() // a new session cancels the pending close…
    vi.advanceTimersByTime(999)
    expect(proc.closeCalls).toBe(0)
    vi.advanceTimersByTime(1) // …and the grace restarts from its detach
    expect(proc.closeCalls).toBe(1)
  })
})

describe('answerServerRequest', () => {
  it('rejects anything it does not know', () => {
    expect(() => answerServerRequest('currentTime/read')).toThrow(JsonRpcError)
  })
})

describe('CodexHostRegistry', () => {
  it('starts one host per home under concurrent acquire and restarts a dead one', async () => {
    const procs: FakeCodexProcess[] = []
    const start = vi.fn(async () => {
      const p = new FakeCodexProcess()
      procs.push(p)
      return new CodexHost(p)
    })
    const registry = new CodexHostRegistry(start)
    const [a, b] = await Promise.all([registry.acquire('/home'), registry.acquire('/home')])
    expect(a).toBe(b)
    expect(start).toHaveBeenCalledTimes(1)
    procs[0]!.crash()
    const c = await registry.acquire('/home')
    expect(c).not.toBe(a)
    expect(start).toHaveBeenCalledTimes(2)
    await registry.closeAll()
    expect(procs[1]!.closeCalls).toBe(1)
  })

  it('forgets a failed start so the next acquire retries', async () => {
    let fail = true
    const registry = new CodexHostRegistry(async () => {
      if (fail) throw new Error('no codex')
      return new CodexHost(new FakeCodexProcess())
    })
    await expect(registry.acquire('/h')).rejects.toThrow('no codex')
    fail = false
    await expect(registry.acquire('/h')).resolves.toBeInstanceOf(CodexHost)
  })
})
