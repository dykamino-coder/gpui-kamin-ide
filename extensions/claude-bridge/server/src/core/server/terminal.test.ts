import { EventEmitter } from 'node:events'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Server } from 'node:http'

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
import { attachTerminalWebSocket, cleanupTerminal } from './terminal'

afterEach(() => {
  cleanupTerminal()
  vi.unstubAllEnvs()
  vi.clearAllMocks()
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
  it('passes the actual shared runtime CODEX_HOME to the shell, overriding inherited personal settings', async () => {
    vi.stubEnv('CODEX_HOME', '/personal/account')
    await connect()
    expect(mocks.spawn.mock.calls[0]![2].env.CODEX_HOME).toBe('/bridge/account')
  })
  it('launches resume directly under an exclusive lease, held past disconnect until process exit', async () => {
    const { ws, pty, process } = await connect('?engine=codex&command=resume')
    expect(mocks.acquire).toHaveBeenCalledOnce()
    expect(mocks.spawn.mock.calls[0]![1].slice(-2)).toEqual(['resume', '--last'])
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
