import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import type { ChildProcessWithoutNullStreams } from 'node:child_process'
import { describe, expect, it, vi } from 'vitest'
import { BRIDGE_CLIENT_INFO, CodexAppServer, type SpawnFn } from './app-server'
import { resolveCodexCommand } from './binary'

type FakeChild = ChildProcessWithoutNullStreams & {
  stdout: PassThrough
  stderr: PassThrough
  sent: any[]
  exitWith: (code: number | null, signal?: NodeJS.Signals | null) => void
  pid: number
}

/** A child process whose stdout is driven by `answer` for each request line. */
function fakeChild(answer: (msg: any, child: FakeChild) => void, { exitOnEof = true } = {}): FakeChild {
  const child = new EventEmitter() as FakeChild
  child.stdin = new PassThrough()
  child.stdout = new PassThrough()
  child.stderr = new PassThrough()
  child.pid = 4242
  child.sent = []
  child.exitWith = (code, signal = null) => setImmediate(() => child.emit('exit', code, signal))
  let buf = ''
  child.stdin.on('data', (chunk: Buffer) => {
    buf += chunk.toString('utf8')
    let nl: number
    while ((nl = buf.indexOf('\n')) >= 0) {
      const msg = JSON.parse(buf.slice(0, nl))
      buf = buf.slice(nl + 1)
      child.sent.push(msg)
      answer(msg, child)
    }
  })
  child.stdin.on('finish', () => {
    if (exitOnEof) child.exitWith(0)
  })
  return child
}

const reply = (child: FakeChild, id: unknown, result: unknown) =>
  child.stdout.write(JSON.stringify({ id, result }) + '\n')
const initOk = (msg: any, child: FakeChild) => {
  if (msg.method === 'initialize') reply(child, msg.id, { userAgent: 'codex/0.160.1' })
}
const codex = { command: 'codex', args: [] }

describe('CodexAppServer', () => {
  it('spawns app-server with overrides and completes the experimental handshake', async () => {
    const child = fakeChild(initOk)
    const spawn = vi.fn<SpawnFn>(() => child)
    const server = await CodexAppServer.start({
      command: { command: 'node', args: ['codex.js'] },
      env: { CODEX_HOME: '/h' },
      cwd: '/w',
      configOverrides: ['-c', 'features.shell_tool=false'],
      spawn,
    })
    expect(spawn).toHaveBeenCalledWith('node', ['codex.js', 'app-server', '-c', 'features.shell_tool=false'], {
      cwd: '/w',
      env: { CODEX_HOME: '/h' },
      detached: process.platform !== 'win32',
      windowsHide: true,
    })
    expect(child.sent).toEqual([
      {
        id: expect.anything(),
        method: 'initialize',
        params: { clientInfo: BRIDGE_CLIENT_INFO, capabilities: { experimentalApi: true, requestAttestation: false } },
      },
      { method: 'initialized' },
    ])
    expect(server.initializeResult).toEqual({ userAgent: 'codex/0.160.1' })
    expect(server.pid).toBe(4242)
    await server.close()
  })

  it('closes gracefully by ending stdin, without killing the tree', async () => {
    const child = fakeChild(initOk)
    const killTree = vi.fn()
    const server = await CodexAppServer.start({ command: codex, env: {}, cwd: '/', spawn: () => child, killTree })
    const exits: unknown[] = []
    server.onExit((e) => exits.push(e))
    const first = server.close()
    expect(server.close()).toBe(first)
    await expect(first).resolves.toEqual({ code: 0, signal: null })
    expect(killTree).not.toHaveBeenCalled()
    expect(exits).toEqual([{ code: 0, signal: null }])
    expect(server.exited).toEqual({ code: 0, signal: null })
    await expect(server.rpc.request('thread/start')).rejects.toThrow()
  })

  it('kills the process tree when stdin EOF is ignored', async () => {
    const child = fakeChild(initOk, { exitOnEof: false })
    const killTree = vi.fn((c: ChildProcessWithoutNullStreams) => (c as FakeChild).exitWith(null, 'SIGKILL'))
    const server = await CodexAppServer.start({ command: codex, env: {}, cwd: '/', spawn: () => child, killTree })
    await expect(server.close(10)).resolves.toEqual({ code: null, signal: 'SIGKILL' })
    expect(killTree).toHaveBeenCalledWith(child)
  })

  it('fails start with the stderr tail when initialize is rejected', async () => {
    const child = fakeChild((msg, c) => {
      if (msg.method !== 'initialize') return
      c.stderr.write('error: unknown feature `bogus`\n')
      setImmediate(() =>
        c.stdout.write(JSON.stringify({ id: msg.id, error: { code: -32600, message: 'bad init' } }) + '\n'),
      )
    })
    await expect(CodexAppServer.start({ command: codex, env: {}, cwd: '/', spawn: () => child })).rejects.toThrow(
      /failed to start: bad init\ncodex stderr:\nerror: unknown feature `bogus`/,
    )
  })

  it('fails start on a spawn error and on an early exit', async () => {
    const missing = fakeChild(() => {})
    setImmediate(() => missing.emit('error', Object.assign(new Error('spawn codex ENOENT'), { code: 'ENOENT' })))
    await expect(
      CodexAppServer.start({
        command: codex,
        env: {},
        cwd: '/',
        spawn: () => missing,
        killTree: () => missing.exitWith(-2),
      }),
    ).rejects.toThrow('spawn codex ENOENT')

    const dying = fakeChild(() => {})
    setImmediate(() => dying.exitWith(1))
    await expect(CodexAppServer.start({ command: codex, env: {}, cwd: '/', spawn: () => dying })).rejects.toThrow(
      'exited (code 1',
    )
  })

  it('caps the stderr buffer', async () => {
    const child = fakeChild(initOk)
    const server = await CodexAppServer.start({
      command: codex,
      env: {},
      cwd: '/',
      spawn: () => child,
      stderrLimitBytes: 4,
    })
    child.stderr.write('abcdefgh')
    await new Promise((r) => setImmediate(r))
    expect(server.stderrTail()).toBe('efgh')
    await server.close()
  })
})

describe('resolveCodexCommand', () => {
  const launcher = 'C:\\Users\\u\\AppData\\Roaming\\npm\\node_modules\\@openai\\codex\\bin\\codex.js'

  it('honours a pinned binary or launcher script', () => {
    expect(resolveCodexCommand({ env: { BRIDGE_CODEX_BIN: '/opt/codex' }, nodePath: 'node' })).toEqual({
      command: '/opt/codex',
      args: [],
    })
    expect(resolveCodexCommand({ env: { BRIDGE_CODEX_BIN: '/opt/codex.mjs' }, nodePath: 'node' })).toEqual({
      command: 'node',
      args: ['/opt/codex.mjs'],
    })
  })

  it('runs the npm launcher through Node on Windows, avoiding cmd.exe quoting', () => {
    const env = { APPDATA: 'C:\\Users\\u\\AppData\\Roaming' }
    expect(
      resolveCodexCommand({ env, platform: 'win32', exists: (f) => f === launcher, nodePath: 'node.exe' }),
    ).toEqual({
      command: 'node.exe',
      args: [launcher],
    })
    expect(resolveCodexCommand({ env, platform: 'win32', exists: () => false })).toEqual({ command: 'codex', args: [] })
  })

  it('uses codex from PATH elsewhere', () => {
    expect(resolveCodexCommand({ env: { APPDATA: '/x' }, platform: 'linux', exists: () => true })).toEqual({
      command: 'codex',
      args: [],
    })
  })
})
