import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { runForegroundCommand } from './shell-foreground'

const spawn = vi.hoisted(() => vi.fn())
vi.mock('node:child_process', () => ({ spawn }))
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); vi.clearAllMocks() })

function child() {
  return Object.assign(new EventEmitter(), {
    stdout: new PassThrough(), stderr: new PassThrough(), pid: 123456,
    killed: false, exitCode: null, signalCode: null,
    kill: vi.fn(function (this: { killed: boolean }) { this.killed = true; return true }),
  })
}

describe('foreground shell deadline', () => {
  it('settles even when a descendant retains the pipes after its shell exits', async () => {
    vi.useFakeTimers()
    vi.spyOn(process, 'kill').mockReturnValue(true)
    const proc = child()
    spawn.mockReturnValue(proc)
    const pending = runForegroundCommand('synthetic', 'synthetic-shell', 100, [], {})
    proc.stdout.write('synthetic output')
    proc.emit('exit', 0, null)
    let settled = false
    void pending.then(() => { settled = true })
    await vi.advanceTimersByTimeAsync(6100)
    expect(settled).toBe(true)
    expect((await pending).content[0]?.text).toContain('timed out')
    expect(vi.getTimerCount()).toBe(0)
  })

  it('cleans up the deadline on ordinary completion', async () => {
    vi.useFakeTimers()
    vi.spyOn(process, 'kill').mockReturnValue(true)
    const proc = child()
    spawn.mockReturnValue(proc)
    const pending = runForegroundCommand('synthetic', 'synthetic-shell', 100, [], {})
    proc.emit('close', 0)
    await pending
    expect(vi.getTimerCount()).toBe(0)
  })
})
