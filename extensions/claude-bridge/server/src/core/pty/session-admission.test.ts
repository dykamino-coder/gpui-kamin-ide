import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
const h = vi.hoisted(() => ({
  root: '',
  spawn: vi.fn(),
  settings: vi.fn(),
  sync: vi.fn(),
  watcher: vi.fn(),
  env: vi.fn(() => ({})),
  stream: vi.fn(async () => ({ enabled: false })),
}))
vi.mock('node-pty', () => ({ spawn: h.spawn }))
vi.mock('./session-settings', () => ({
  get SESSIONS_BASE() {
    return h.root
  },
  sanitizeDirName: () => 'test',
  writeSessionSettings: h.settings,
  writeSessionClaudeMd: vi.fn(),
  applySyncData: h.sync,
}))
vi.mock('./session-env', () => ({ buildSessionEnv: h.env, modelForResume: (s: string) => s }))
vi.mock('./session-plugin-args', () => ({ buildSessionClaudeArgs: () => [] }))
vi.mock('./session-resume-helpers', () => ({
  findOrRecreateSettingsDir: () => h.root,
  xbasename: (s: string) => path.basename(s),
  repairTranscriptForResume: vi.fn(),
  resolveNewestInChain: () => null,
  lastModelEntryForResume: () => null,
}))
vi.mock('./jsonl-watcher', () => ({
  JsonlWatcher: class {
    start = h.watcher
    stop = vi.fn()
  },
}))
vi.mock('./session-io', () => ({
  sendToClient: vi.fn(),
  writeInputToSession: vi.fn(),
  submitTextToSession: vi.fn(),
  resizeSessionTerminal: vi.fn(),
  attachStartupAutoResponder: vi.fn(),
  attachOutputDebounce: vi.fn(),
  clearInputThrottle: vi.fn(),
}))
vi.mock('../proxy/native-mitm', () => ({ startNativeMitm: vi.fn() }))
vi.mock('../proxy/streaming-settings', () => ({ getStreamingSettings: h.stream }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), infoLog: vi.fn(), warnLog: vi.fn() }))
vi.mock('./transcript-archive', () => ({ archiveTranscriptForSession: vi.fn(), dropArchivedTranscript: vi.fn() }))
let core: typeof import('./session-core')
const socket = { readyState: 1, send: vi.fn() } as any
beforeEach(async () => {
  vi.resetModules()
  vi.stubEnv('BRIDGE_MAX_SESSIONS', '2')
  h.root = fs.mkdtempSync(path.join(os.tmpdir(), 'admission-test-'))
  h.spawn.mockReset().mockImplementation(() => {
    const exits = new Set<(event: { exitCode: number }) => void>()
    return {
      pid: 1,
      onExit: vi.fn((callback: (event: { exitCode: number }) => void) => {
        exits.add(callback)
        return { dispose: () => exits.delete(callback) }
      }),
      kill: vi.fn(() =>
        queueMicrotask(() => {
          for (const callback of [...exits]) callback({ exitCode: 0 })
        }),
      ),
    }
  })
  h.settings.mockReset()
  h.sync.mockReset()
  h.watcher.mockReset()
  h.env.mockReset().mockReturnValue({})
  h.stream.mockReset().mockResolvedValue({ enabled: false })
  core = await import('./session-core')
})

afterEach(() => {
  for (const session of core.sessions.values()) {
    if (session.pty && !vi.isMockFunction(session.pty.kill)) session.pty.kill()
  }
  core.sessions.clear()
  if (
    path.dirname(path.resolve(h.root)) !== path.resolve(os.tmpdir()) ||
    !path.basename(h.root).startsWith('admission-test-')
  )
    throw new Error('unexpected fixture root')
  fs.rmSync(h.root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 })
  vi.unstubAllEnvs()
  vi.clearAllTimers()
  vi.useRealTimers()
})
describe('INC-2026-0012 actual core admission', () => {
  it('fake CLI delayed startup respects actual core cancellation and releases admission', async () => {
    vi.stubEnv('NODE_ENV', 'test')
    vi.stubEnv('BRIDGE_DEV_FAKE_CLI', '1')
    vi.stubEnv('BRIDGE_FAKE_CLI_HOME', os.homedir())
    vi.stubEnv('BRIDGE_FAKE_CLI_STARTUP_DELAY_MS', '30000')
    const controller = new AbortController()
    const starting = core.createSession(socket, 'tester', 'token', {}, controller.signal)
    controller.abort()
    await expect(starting).rejects.toThrow()
    expect(h.spawn).not.toHaveBeenCalled()
    expect(h.settings).not.toHaveBeenCalled()
    expect(core.sessions.size).toBe(0)
    vi.stubEnv('BRIDGE_FAKE_CLI_STARTUP_DELAY_MS', '0')
    await core.createSession(socket, 'tester', 'token')
    expect(h.spawn.mock.calls[0]![0]).toBe(process.execPath)
    expect(h.spawn.mock.calls[0]![1][0]).toContain('fake-claude.mjs')
  })
  it('production session spawn cannot select fake even with the switch enabled', async () => {
    vi.stubEnv('NODE_ENV', 'production')
    vi.stubEnv('BRIDGE_DEV_FAKE_CLI', '1')
    await core.createSession(socket, 'tester', 'token')
    expect(h.spawn.mock.calls[0]![0]).toBe(process.platform === 'win32' ? 'claude.cmd' : 'claude')
    expect(h.spawn.mock.calls[0]![1]).toEqual([])
  })

  it('reserves capacity while the real snapshot lock blocks concurrent startup', async () => {
    // Import the same real lock instance as the reset core module.
    const { withUserSyncLock } = await import('../sync/lock')
    let release!: () => void
    const blocked = withUserSyncLock(
      'synthetic-hash',
      () =>
        new Promise<void>((resolve) => {
          release = resolve
        }),
    )
    await Promise.resolve()
    const created = Promise.allSettled(
      Array.from({ length: 4 }, () => core.createSession(socket, 'tester', 'token', { bearerHash: 'synthetic-hash' })),
    )
    release()
    await blocked
    const results = await created
    expect(results.filter((r) => r.status === 'fulfilled')).toHaveLength(2)
    expect(h.spawn).toHaveBeenCalledTimes(2)
    expect(core.sessions.size).toBe(2)
  })
  it('applies the per-token cap to cold resume in the shared core', async () => {
    for (let i = 0; i < 10; i++) core.sessions.set(`existing-${i}`, { tokenId: 'token', state: 'running' } as any)
    // Raise global budget by reloading, leaving the per-token budget authoritative.
    vi.stubEnv('BRIDGE_MAX_SESSIONS', '200')
    vi.resetModules()
    core = await import('./session-core')
    for (let i = 0; i < 10; i++) core.sessions.set(`existing-${i}`, { tokenId: 'token', state: 'running' } as any)
    await expect(
      core.createSession(socket, 'tester', 'token', { resumeConversationId: 'conversation' }),
    ).rejects.toThrow('Max sessions')
    expect(h.spawn).not.toHaveBeenCalled()
  })
  it.each(['settings', 'sync', 'env', 'spawn'] as const)(
    'releases reservations after %s startup failure',
    async (stage) => {
      h[stage].mockImplementationOnce(() => {
        throw new Error('synthetic failure')
      })
      await expect(core.createSession(socket, 'tester', 'token', { bearerHash: 'synthetic-hash' })).rejects.toThrow(
        'synthetic failure',
      )
      const next = await core.createSession(socket, 'tester', 'token', { bearerHash: 'synthetic-hash' })
      expect(next.state).toBe('running')
      const last = await core.createSession(socket, 'tester', 'other-token')
      expect(last.state).toBe('running')
      expect(core.sessions.size).toBe(2)
    },
  )
  it('cancels a blocked startup before spawn and releases the reservation', async () => {
    const { withUserSyncLock } = await import('../sync/lock')
    let release!: () => void
    const blocked = withUserSyncLock(
      'synthetic-hash',
      () =>
        new Promise<void>((resolve) => {
          release = resolve
        }),
    )
    await Promise.resolve()
    const controller = new AbortController()
    const rejected = expect(
      core.createSession(socket, 'tester', 'token', { bearerHash: 'synthetic-hash' }, controller.signal),
    ).rejects.toThrow()
    controller.abort()
    release()
    await blocked
    await rejected
    expect(h.spawn).not.toHaveBeenCalled()
    await core.createSession(socket, 'tester', 'token')
    await core.createSession(socket, 'tester', 'other')
    expect(core.sessions.size).toBe(2)
  })
  it('replaces a restart slot at full quota without a third PTY', async () => {
    const first = await core.createSession(socket, 'tester', 'token')
    first.cliConversationId = 'conversation'
    await core.createSession(socket, 'tester', 'other')
    const replacement = await core.restartWithModel(first.id, 'test-model', socket, 'tester', 'token')
    expect(core.sessions.size).toBe(2)
    expect(core.sessions.has(first.id)).toBe(false)
    expect(core.sessions.get(replacement.id)).toBe(replacement)
  })
  it('reattaches a live session at full quota without admission', async () => {
    const first = await core.createSession(socket, 'tester', 'token')
    await core.createSession(socket, 'tester', 'other')
    core.reattachSession(first, socket)
    expect(core.sessions.size).toBe(2)
    expect(h.spawn).toHaveBeenCalledTimes(2)
  })
  it('counts per-token reservations across distinct cold conversations', async () => {
    vi.stubEnv('BRIDGE_MAX_SESSIONS', '200')
    vi.resetModules()
    core = await import('./session-core')
    const results = await Promise.allSettled(
      Array.from({ length: 11 }, (_, i) =>
        core.createSession(socket, 'tester', 'token', {
          resumeConversationId: `conv-${i}`,
          bearerHash: 'synthetic-hash',
        }),
      ),
    )
    expect(results.filter((r) => r.status === 'fulfilled')).toHaveLength(10)
    expect(h.spawn).toHaveBeenCalledTimes(10)
  })
  it('tears down registration after watcher failure and returns capacity', async () => {
    vi.useFakeTimers()
    h.watcher.mockImplementationOnce(() => {
      throw new Error('watcher failed')
    })
    await expect(core.createSession(socket, 'tester', 'token')).rejects.toThrow('watcher failed')
    expect([...core.sessions.values()].every((s) => s.state === 'exiting')).toBe(true)
    vi.advanceTimersByTime(5001)
    expect(core.sessions.size).toBe(0)
    await core.createSession(socket, 'tester', 'token')
    await core.createSession(socket, 'tester', 'other')
    expect(core.sessions.size).toBe(2)
  })
  it.skipIf(process.platform !== 'linux' || process.env.BRIDGE_PTY_ACCEPTANCE !== '1')(
    'disposable Linux PTY gate: bounds real spawns and restores capacity after failure',
    async () => {
      const native = await vi.importActual<typeof import('node-pty')>('node-pty')
      const executable = path.join(h.root, 'claude')
      fs.writeFileSync(executable, '#!/bin/sh\nexec /bin/sleep 60\n', { mode: 0o700 })
      h.env.mockReturnValue({ PATH: h.root, HOME: h.root } as any)
      const pties: ReturnType<typeof native.spawn>[] = []
      h.spawn.mockImplementation((...args: Parameters<typeof native.spawn>) => {
        const pty = native.spawn(...args)
        pties.push(pty)
        return pty
      })
      const stop = async () => {
        await Promise.all(
          pties.splice(0).map(
            (pty) =>
              new Promise<void>((resolve) => {
                const sub = pty.onExit(() => {
                  sub.dispose()
                  resolve()
                })
                pty.kill()
              }),
          ),
        )
      }
      try {
        const results = await Promise.allSettled(
          Array.from({ length: 4 }, () =>
            core.createSession(socket, 'tester', 'token', { bearerHash: 'synthetic-hash' }),
          ),
        )
        expect(results.filter((r) => r.status === 'fulfilled')).toHaveLength(2)
        expect(pties).toHaveLength(2)
        for (const pty of pties) expect(() => process.kill(pty.pid, 0)).not.toThrow()
        await stop()
        expect(core.sessions.size).toBe(0)
        h.settings.mockImplementationOnce(() => {
          throw new Error('synthetic settings failure')
        })
        await expect(core.createSession(socket, 'tester', 'token')).rejects.toThrow()
        expect(core.sessions.size).toBe(0)
        h.env.mockReturnValue({ PATH: h.root, HOME: h.root } as any)
        await core.createSession(socket, 'tester', 'token')
        await core.createSession(socket, 'tester', 'other')
        expect(pties).toHaveLength(2)
        await stop()
        expect(core.sessions.size).toBe(0)
      } finally {
        await stop()
      }
    },
    15_000,
  )
})
