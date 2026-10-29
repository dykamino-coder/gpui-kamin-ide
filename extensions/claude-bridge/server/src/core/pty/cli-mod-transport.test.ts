import { afterEach, describe, expect, it, vi } from 'vitest'
import http from 'node:http'
import fs from 'node:fs'
import { CliModTransport, createCliModTransport } from './cli-mod-transport'

const transports: CliModTransport[] = []
const versionProbe = vi.hoisted(() => vi.fn())
vi.mock('node:child_process', () => ({ execFile: versionProbe }))
afterEach(() => {
  transports.splice(0).forEach((t) => t.dispose())
  vi.restoreAllMocks()
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
  versionProbe.mockReset()
})
async function setup() {
  const transport = new CliModTransport('session-a')
  transports.push(transport)
  await transport.listen()
  const call = (route: string, extra: Record<string, unknown> = {}) =>
    new Promise<any>((resolve, reject) => {
      const req = http.request({ socketPath: transport.socketPath, path: route, method: 'POST' }, (res) => {
        let body = ''
        res.on('data', (chunk) => {
          body += chunk
        })
        res.on('end', () => resolve({ status: res.statusCode, ...(body ? JSON.parse(body) : {}) }))
      })
      req.on('error', reject)
      req.end(
        JSON.stringify({
          protocol: 1,
          session: 'session-a',
          instance: 'one',
          version: '2.1.312',
          turnId: null,
          ...extra,
        }),
      )
    })
  return { transport, call }
}

// The transport is Linux-only in production (createCliModTransport); Windows has no AF_UNIX path listen.
const socketIt = it.skipIf(process.platform === 'win32')

describe('private mod transport', () => {
  it('allows an explicit opt-out without probing the CLI', async () => {
    vi.stubEnv('BRIDGE_CLI_MOD', '0')
    expect(await createCliModTransport('unused')).toBeUndefined()
    expect(versionProbe).not.toHaveBeenCalled()
  })
  it.each(['win32', 'darwin', 'freebsd'])('falls back without probing or opening a socket on %s', async (platform) => {
    vi.stubEnv('BRIDGE_CLI_MOD', undefined)
    vi.stubGlobal('process', { ...process, platform })
    expect(await createCliModTransport('unused')).toBeUndefined()
    expect(versionProbe).not.toHaveBeenCalled()
  })
  socketIt('enables the pinned CLI by default, using the session invocation and environment', async () => {
    vi.stubEnv('BRIDGE_CLI_MOD', undefined)
    vi.stubGlobal('process', { ...process, platform: 'linux' })
    versionProbe.mockImplementation((_cmd, _args, _options, callback) => callback(null, '2.1.312 (Claude Code)\n'))
    const cli = { command: '/cli/claude', prefix: ['--custom'], env: { PATH: '/cli' } }
    const transport = await createCliModTransport('default', cli)
    expect(transport).toBeDefined()
    transports.push(transport!)
    expect(versionProbe).toHaveBeenCalledWith(
      cli.command,
      ['--custom', '--version'],
      expect.objectContaining({ env: cli.env, timeout: 5000 }),
      expect.any(Function),
    )
  })
  it.each(['2.1.311 (Claude Code)', '2.1.313 (Claude Code)', 'unknown', ''])(
    'falls back cleanly for CLI %j',
    async (version) => {
      vi.stubEnv('BRIDGE_CLI_MOD', undefined)
      vi.stubGlobal('process', { ...process, platform: 'linux' })
      versionProbe.mockImplementation((_cmd, _args, _options, callback) =>
        callback(version ? null : new Error('unavailable'), version),
      )
      expect(await createCliModTransport('unused')).toBeUndefined()
    },
  )
  it('falls back when the version probe throws synchronously', async () => {
    vi.stubGlobal('process', { ...process, platform: 'linux' })
    versionProbe.mockImplementation(() => {
      throw new Error('invalid invocation')
    })
    expect(await createCliModTransport('unused')).toBeUndefined()
  })
  socketIt('disposes a transport whose socket cannot listen', async () => {
    versionProbe.mockImplementation((_cmd, _args, _options, callback) => callback(null, '2.1.312 (Claude Code)'))
    vi.spyOn(CliModTransport.prototype, 'listen').mockRejectedValue(new Error('socket unavailable'))
    const dispose = vi.spyOn(CliModTransport.prototype, 'dispose')
    expect(await createCliModTransport('unused')).toBeUndefined()
    expect(dispose).toHaveBeenCalledOnce()
  })
  socketIt('requires a session/version handshake and protects socket permissions', async () => {
    const { transport, call } = await setup()
    expect((await call('/poll')).status).toBe(409)
    expect((await call('/hello', { session: 'other' })).status).toBe(409)
    expect((await call('/hello', { version: '2.1.313' })).status).toBe(409)
    expect(transport.healthy()).toBe(false)
    expect((await call('/hello')).status).toBe(200)
    expect(transport.healthy()).toBe(true)
    expect(fs.statSync(transport.directory).mode & 0o777).toBe(0o700)
    expect(fs.statSync(transport.socketPath).mode & 0o777).toBe(0o600)
  })
  socketIt('claims once, keeps heartbeat alive for queued prompts, and forwards boundaries in order', async () => {
    const { transport, call } = await setup()
    const turns: Array<string | null> = []
    transport.onTurn = (turn) => turns.push(turn)
    await call('/hello')
    expect(transport.submit('Привет\n世界', vi.fn())).toBe(true)
    const claimed = await call('/poll')
    expect(claimed.commands).toEqual([{ kind: 'submit', id: 1, text: 'Привет\n世界' }])
    expect((await call('/poll')).commands).toEqual([])
    await call('/poll', { turns: [{ turnId: 'a' }, { turnId: null }] })
    expect(turns.slice(-3)).toEqual(['a', null, null])
    expect(transport.abort('a', vi.fn())).toBe(true)
    expect((await call('/poll')).commands).toEqual([{ kind: 'abort', id: 2, turnId: 'a' }])
  })
  socketIt('falls back only for unclaimed commands and reports uncertain claimed delivery', async () => {
    const { transport, call } = await setup()
    const fallback = vi.fn(),
      uncertain = vi.fn()
    transport.onUncertain = uncertain
    await call('/hello')
    transport.submit('claimed', fallback)
    await call('/poll')
    transport.submit('not claimed', fallback)
    const now = Date.now()
    vi.spyOn(Date, 'now').mockReturnValue(now + 4000)
    expect(transport.healthy()).toBe(false)
    expect(fallback).toHaveBeenCalledOnce()
    expect(uncertain).toHaveBeenCalledOnce()
    expect(transport.submit('next', fallback)).toBe(false)
    expect((await call('/poll')).status).toBe(409)
  })
  socketIt('bounds queue/text and cleans only its own paths', async () => {
    const { transport, call } = await setup()
    await call('/hello')
    expect(transport.submit('x'.repeat(131_073), vi.fn())).toBe(false)
    for (let n = 0; n < 64; n++) expect(transport.submit('x', vi.fn())).toBe(true)
    expect(transport.submit('full', vi.fn())).toBe(false)
    transport.dispose()
    expect(fs.existsSync(transport.directory)).toBe(false)
  })
})
