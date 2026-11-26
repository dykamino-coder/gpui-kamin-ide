import { EventEmitter } from 'node:events'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createServer, type Server } from 'node:http'
import { WebSocket } from 'ws'

const mocks = vi.hoisted(() => ({
  route: null as any,
  spawn: vi.fn(),
  release: vi.fn(async () => {}),
  acquire: vi.fn(),
}))
vi.mock('./ws', () => ({
  registerWsRoute: (_path: string, wss: unknown) => {
    mocks.route = wss
  },
}))
vi.mock('node-pty', () => ({ spawn: mocks.spawn }))
vi.mock('../codex/codex-runtime', () => ({
  codexRuntime: () => ({ home: '/bridge/account', acquireTerminal: mocks.acquire }),
}))
// The host's real launcher (for example node + codex.js on Windows) would prefix the argv under test.
vi.mock('../codex/binary', () => ({ resolveCodexCommand: () => ({ command: 'codex', args: [] }) }))
import { attachTerminalWebSocket, cleanupTerminal } from './terminal'

afterEach(() => {
  cleanupTerminal()
  vi.clearAllTimers()
  vi.useRealTimers()
  vi.unstubAllEnvs()
  vi.clearAllMocks()
})

describe('terminal output at process exit', () => {
  it('delivers the pending tail to a real WebSocket client before exit', async () => {
    const events = new EventEmitter()
    mocks.spawn.mockReturnValue({
      kill: vi.fn(),
      onData: (fn: (...args: any[]) => void) => events.on('data', fn),
      onExit: (fn: (...args: any[]) => void) => events.on('exit', fn),
    })
    mocks.acquire.mockResolvedValue(mocks.release)
    const server = createServer()
    attachTerminalWebSocket(server)
    const route = mocks.route
    server.on('upgrade', (request, socket, head) => {
      route.handleUpgrade(request, socket, head, (ws: WebSocket) => route.emit('connection', ws, request))
    })
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
    const port = (server.address() as { port: number }).port
    const client = new WebSocket(`ws://127.0.0.1:${port}/ws/terminal?engine=codex`)
    const frames: unknown[] = []
    client.on('message', (raw) => frames.push(JSON.parse(String(raw))))
    try {
      await new Promise<void>((resolve, reject) => client.once('open', resolve).once('error', reject))
      await vi.waitFor(() => expect(mocks.spawn).toHaveBeenCalledOnce())
      events.emit('data', 'final diagnostic\r\n')
      events.emit('exit', { exitCode: 1 })
      await vi.waitFor(() => expect(client.readyState).toBe(WebSocket.CLOSED))
      expect(frames).toEqual([
        { type: 'output', data: 'final diagnostic\r\n' },
        { type: 'exit', code: 1 },
      ])
      expect(mocks.release).toHaveBeenCalledOnce()
    } finally {
      client.terminate()
      await new Promise<void>((resolve) => server.close(() => resolve()))
      route.close()
    }
  })

  it.each(['', '?engine=codex'])('delivers the PTY tail once before exit for %s', async (query) => {
    const { ws, pty, process } = await connect(query)
    vi.useFakeTimers()
    process.emit('data', 'last diagnostic: ')
    process.emit('data', 'startup failed\r\n')
    expect(ws.send).not.toHaveBeenCalled()
    process.emit('exit', { exitCode: 1 })
    expect(ws.send.mock.calls.map(([frame]) => JSON.parse(frame))).toEqual([
      { type: 'output', data: 'last diagnostic: startup failed\r\n' },
      { type: 'exit', code: 1 },
    ])
    expect(ws.send.mock.invocationCallOrder.at(-1)).toBeLessThan(ws.close.mock.invocationCallOrder[0]!)
    expect(vi.getTimerCount()).toBe(0)
    vi.runAllTimers()
    process.emit('data', 'late event')
    expect(ws.send).toHaveBeenCalledTimes(2)
    ws.emit('close')
    expect(pty.kill).not.toHaveBeenCalled()
    if (query) expect(mocks.release).toHaveBeenCalledOnce()
    else expect(mocks.acquire).not.toHaveBeenCalled()
  })

  it('does not repeat output already sent by the debounce timer', async () => {
    const { ws, process } = await connect()
    vi.useFakeTimers()
    process.emit('data', 'already delivered')
    vi.advanceTimersByTime(8)
    process.emit('exit', { exitCode: 0 })
    vi.runAllTimers()
    expect(ws.send.mock.calls.map(([frame]) => JSON.parse(frame))).toEqual([
      { type: 'output', data: 'already delivered' },
      { type: 'exit', code: 0 },
    ])
    expect(vi.getTimerCount()).toBe(0)
  })

  it('discards a disconnected client tail and keeps the lease until the process actually exits', async () => {
    const { ws, pty, process } = await connect('?engine=codex')
    vi.useFakeTimers()
    process.emit('data', 'undelivered')
    ws.readyState = 3
    ws.emit('error', new Error('client gone'))
    ws.emit('close')
    process.emit('data', 'late output')
    expect(pty.kill).toHaveBeenCalledOnce()
    expect(mocks.release).not.toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
    process.emit('exit', { exitCode: 1 })
    process.emit('exit', { exitCode: 1 })
    vi.runAllTimers()
    expect(mocks.release).toHaveBeenCalledOnce()
    expect(ws.send).not.toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('clears pending timers even if the socket closed before its close handler', async () => {
    const { ws, process } = await connect()
    vi.useFakeTimers()
    process.emit('data', 'unsendable')
    ws.readyState = 3
    process.emit('exit', { exitCode: 1 })
    expect(vi.getTimerCount()).toBe(0)
    expect(ws.send).not.toHaveBeenCalled()
  })

  it('does not send exit on an errored socket that still reports OPEN', async () => {
    const { ws, process } = await connect('?engine=codex')
    vi.useFakeTimers()
    process.emit('data', 'pending tail')
    ws.emit('error', new Error('transport failed'))
    expect(mocks.release).not.toHaveBeenCalled()
    process.emit('exit', { exitCode: 1 })
    expect(mocks.release).toHaveBeenCalledOnce()
    expect(ws.send).not.toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
  })
})

async function connect(query = '') {
  const process = new EventEmitter()
  const pty = {
    kill: vi.fn(),
    write: vi.fn(),
    resize: vi.fn(),
    onData: (fn: (...args: any[]) => void) => process.on('data', fn),
    onExit: (fn: (...args: any[]) => void) => process.on('exit', fn),
  }
  mocks.spawn.mockReturnValue(pty)
  mocks.acquire.mockResolvedValue(mocks.release)
  const ws = Object.assign(new EventEmitter(), { readyState: 1, send: vi.fn(), close: vi.fn() })
  attachTerminalWebSocket(null as unknown as Server)
  mocks.route.emit('connection', ws, { url: `/ws/terminal${query}` })
  await vi.waitFor(() => expect(mocks.spawn).toHaveBeenCalledOnce())
  return { ws, pty, process }
}

describe('terminal launch contract', () => {
  it.each(['interactive', 'usage'])('launches Codex %s in the leased foreground process', async (command) => {
    await connect(`?engine=codex&command=${command}`)
    expect(mocks.acquire).toHaveBeenCalledOnce()
    expect(mocks.spawn.mock.calls[0]![1]).toEqual(['--no-daemon'])
  })
  it('keeps the read-only Codex login status invocation unchanged', async () => {
    await connect('?engine=codex&command=status')
    expect(mocks.spawn.mock.calls[0]![1]).toEqual(['login', 'status'])
  })
  it('passes the actual shared runtime CODEX_HOME to the shell, overriding inherited personal settings', async () => {
    vi.stubEnv('CODEX_HOME', '/personal/account')
    await connect()
    expect(mocks.spawn.mock.calls[0]![2].env.CODEX_HOME).toBe('/bridge/account')
  })
  it('launches resume directly under an exclusive lease, held past disconnect until process exit', async () => {
    const { ws, pty, process } = await connect('?engine=codex&command=resume')
    expect(mocks.acquire).toHaveBeenCalledOnce()
    expect(mocks.spawn.mock.calls[0]![1]).toEqual(['--no-daemon', 'resume', '--last'])
    expect(mocks.spawn.mock.calls[0]![2].env.CODEX_HOME).toBe('/bridge/account')
    ws.emit('close')
    expect(pty.kill).toHaveBeenCalledOnce()
    expect(mocks.release).not.toHaveBeenCalled()
    process.emit('exit', { exitCode: 0 })
    await vi.waitFor(() => expect(mocks.release).toHaveBeenCalledOnce())
  })
  it('never spawns a CLI when another account writer owns the home', async () => {
    mocks.acquire.mockRejectedValueOnce(new Error('home is busy'))
    const ws = Object.assign(new EventEmitter(), { readyState: 1, send: vi.fn(), close: vi.fn() })
    attachTerminalWebSocket(null as unknown as Server)
    mocks.route.emit('connection', ws, { url: '/ws/terminal?engine=codex' })
    await vi.waitFor(() => expect(ws.close).toHaveBeenCalledOnce())
    expect(mocks.spawn).not.toHaveBeenCalled()
    expect(JSON.parse(ws.send.mock.calls[0]![0]).data).toContain('home is busy')
  })
})
