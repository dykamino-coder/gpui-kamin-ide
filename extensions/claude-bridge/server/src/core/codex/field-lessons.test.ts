// Field lessons of the Codex agent driver (codex_driver.py: the same
// `codex app-server` protocol, run for hundreds of agent sessions), each pinned
// to the Bridge mechanism that closes it. End-to-end counterparts with a real
// app-server process live in pty/session-codex.e2e.test.ts.
import { spawn } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { killProcessTree } from './app-server'
import { acquireCodexHomeLock, CODEX_HOME_LOCK_FILE, CodexHomeBusyError } from './codex-home-lock'
import {
  CodexHost,
  CodexHostRegistry,
  CodexRestartPausedError,
  CodexThreadBusyError,
  type ThreadSink,
} from './codex-host'
import { CodexAgentSession } from './codex-session'
import { FakeCodexProcess, flush } from './fixtures/fake-process'

const sink = (): ThreadSink => ({ onNotification: () => undefined, onExit: () => undefined })
const thread = (id: string) => ({ thread: { id, model: 'gpt-5.5', cwd: '/w', path: null }, model: 'gpt-5.5' })

afterEach(() => vi.useRealTimers())

describe('(1) one writer per codex thread', () => {
  it('a second session cannot attach to or resume a thread that is open', async () => {
    const proc = new FakeCodexProcess({ 'thread/resume': (p) => thread(p.threadId) })
    const host = new CodexHost(proc)
    const owner = await CodexAgentSession.start({ host, cwd: '/w', resumeThreadId: 't1' })
    expect(host.isAttached('t1')).toBe(true)
    await expect(CodexAgentSession.start({ host, cwd: '/w', resumeThreadId: 't1' })).rejects.toBeInstanceOf(
      CodexThreadBusyError,
    )
    // codex was asked to resume the thread only once.
    expect(proc.requests('thread/resume')).toHaveLength(1)
    expect(() => host.attach('t1', sink())).toThrow(CodexThreadBusyError)
    // Once the owner is gone the thread can be taken again.
    await owner.stop()
    await expect(CodexAgentSession.start({ host, cwd: '/w', resumeThreadId: 't1' })).resolves.toBeTruthy()
  })

  it('a second Bridge process cannot start an app-server on the same CODEX_HOME', () => {
    const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-codex-lock-'))
    try {
      const first = acquireCodexHomeLock(home, { pid: 101, host: 'bridge-a' })
      expect(() => acquireCodexHomeLock(home, { pid: 202, host: 'bridge-b' })).toThrow(CodexHomeBusyError)
      // Same host, owner process gone: taken over.
      const dead = acquireCodexHomeLock(home, { pid: 303, host: 'bridge-a', isAlive: () => false })
      expect(JSON.parse(fs.readFileSync(path.join(home, CODEX_HOME_LOCK_FILE), 'utf8'))).toMatchObject({ pid: 303 })
      first.release() // not the owner any more: leaves the lock alone
      expect(fs.existsSync(path.join(home, CODEX_HOME_LOCK_FILE))).toBe(true)
      dead.release()
      expect(fs.existsSync(path.join(home, CODEX_HOME_LOCK_FILE))).toBe(false)
    } finally {
      fs.rmSync(home, { recursive: true, force: true })
    }
  })

  it('a lock whose heartbeat stopped (crashed owner on another host) is taken over', () => {
    const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-codex-lock-'))
    try {
      acquireCodexHomeLock(home, { pid: 1, host: 'old-container', heartbeatMs: 60_000 })
      const later = Date.now() + 10 * 60_000
      const lock = acquireCodexHomeLock(home, { pid: 2, host: 'new-container', now: () => later })
      expect(JSON.parse(fs.readFileSync(path.join(home, CODEX_HOME_LOCK_FILE), 'utf8'))).toMatchObject({
        host: 'new-container',
      })
      lock.release()
    } finally {
      fs.rmSync(home, { recursive: true, force: true })
    }
  })
})

describe('(2) a finished turn is not a finished process', () => {
  it('turn/completed keeps the app-server and its threads; only a process exit counts as a crash', async () => {
    let turns = 0
    const proc = new FakeCodexProcess({
      'thread/start': () => thread('t1'),
      'turn/start': () => ({ turn: { id: `u${++turns}`, status: 'inProgress', error: null } }),
    })
    const host = new CodexHost(proc)
    const registry = new CodexHostRegistry(async () => host)
    expect(await registry.acquire('/h')).toBe(host)
    const session = await CodexAgentSession.start({ host, cwd: '/w' })
    const exits: unknown[] = []
    session.events.subscribe((e) => {
      if (e.kind === 'session.exited') exits.push(e)
    })
    await session.send('work')
    proc.notify('turn/completed', { threadId: 't1', turn: { id: 'u1', status: 'completed', error: null } })
    await flush()
    expect(host.alive).toBe(true)
    expect(host.crashed).toBe(false)
    expect(exits).toEqual([])
    expect(await registry.acquire('/h')).toBe(host)
    // An idle close is intentional, not a crash.
    await host.close()
    expect(host.crashed).toBe(false)
  })
})

describe('(3) app-server restarts are rate limited', () => {
  function registry(now: { t: number }) {
    const procs: FakeCodexProcess[] = []
    const start = vi.fn(async () => {
      const p = new FakeCodexProcess()
      procs.push(p)
      return new CodexHost(p)
    })
    return { reg: new CodexHostRegistry(start, { now: () => now.t }), procs, start }
  }

  it('two crashes right after start pause restarts, then allow them again', async () => {
    const now = { t: 0 }
    const { reg, procs, start } = registry(now)
    await reg.acquire('/h')
    now.t += 30_000
    procs[0]!.crash()
    await reg.acquire('/h') // one quick crash: restarted
    now.t += 30_000
    procs[1]!.crash()
    const paused = reg.acquire('/h')
    await expect(paused).rejects.toBeInstanceOf(CodexRestartPausedError)
    await expect(paused).rejects.toThrow('restarts are paused')
    expect(start).toHaveBeenCalledTimes(2)
    now.t += 5 * 60_000 + 1
    await expect(reg.acquire('/h')).resolves.toBeInstanceOf(CodexHost)
    expect(start).toHaveBeenCalledTimes(3)
  })

  it('a long-lived app-server that crashes is restarted at once; many crashes in a window still pause', async () => {
    const now = { t: 0 }
    const { reg, procs } = registry(now)
    for (let i = 0; i < 4; i++) {
      await reg.acquire('/h')
      now.t += 2 * 60_000 + 1 // lived longer than the storm window
      procs[i]!.crash()
    }
    await expect(reg.acquire('/h')).resolves.toBeInstanceOf(CodexHost) // 4 crashes in 10 min: still allowed
    now.t += 2 * 60_000 + 1
    procs[4]!.crash()
    await expect(reg.acquire('/h')).rejects.toBeInstanceOf(CodexRestartPausedError) // the 5th pauses
  })
})

describe('(4) stopping kills the whole codex process tree', () => {
  it('a grandchild of the app-server process dies with it', async () => {
    // Parent (like the npm launcher) starts a long-lived child (like the native
    // binary) and reports its pid, then both just wait.
    const grandchild = `setInterval(() => {}, 1000)`
    const parentCode = `
      const { spawn } = require('node:child_process')
      const c = spawn(process.execPath, ['-e', ${JSON.stringify(grandchild)}], { stdio: 'ignore' })
      process.stdout.write(String(c.pid) + '\\n')
      setInterval(() => {}, 1000)
    `
    const parent = spawn(process.execPath, ['-e', parentCode], {
      stdio: ['pipe', 'pipe', 'pipe'],
      detached: process.platform !== 'win32', // as CodexAppServer spawns it
      windowsHide: true,
    })
    const pid = await new Promise<number>((resolve, reject) => {
      parent.stdout.once('data', (d: Buffer) => resolve(Number(String(d).trim())))
      parent.once('error', reject)
    })
    const alive = (p: number) => {
      try {
        process.kill(p, 0)
        return true
      } catch {
        return false
      }
    }
    expect(alive(pid)).toBe(true)
    const exited = new Promise((r) => parent.once('exit', r))
    killProcessTree(parent)
    await exited
    await vi.waitFor(() => expect(alive(pid)).toBe(false), { timeout: 10_000, interval: 100 })
  }, 20_000)
})

describe('(6) steering a running turn and queuing around it', () => {
  it('a message sent while turn/start is still in flight steers that turn instead of racing a second one', async () => {
    let turns = 0
    const proc = new FakeCodexProcess({
      'thread/start': () => thread('t1'),
      'turn/start': () => ({ turn: { id: `u${++turns}`, status: 'inProgress', error: null } }),
      'turn/steer': (p) => ({ turnId: p.expectedTurnId }),
    })
    const session = await CodexAgentSession.start({ host: new CodexHost(proc), cwd: '/w' })
    // Both in the same tick: the turn/start answer has not arrived yet.
    const first = session.send('first', 's1')
    const second = session.send('second', 's2')
    await Promise.all([first, second])
    expect(proc.requests('turn/start')).toHaveLength(1)
    expect(proc.requests('turn/steer')).toEqual([
      expect.objectContaining({ expectedTurnId: 'u1', clientUserMessageId: 's2' }),
    ])
  })
})
