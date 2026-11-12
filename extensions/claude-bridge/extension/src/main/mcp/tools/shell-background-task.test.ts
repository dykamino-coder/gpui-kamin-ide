import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { describe, expect, it, vi } from 'vitest'
import {
  bashExecute,
  powerShellExecute,
  backgroundShellSnapshot,
  localBackgroundShellSnapshot,
  readBackgroundOutput,
  stopBackgroundShellForOwner,
} from './shell'

const mock = vi.hoisted(() => ({ foreground: vi.fn(), spawn: vi.fn() }))
vi.mock('./shell-foreground', () => ({
  runForegroundCommand: mock.foreground,
}))
vi.mock('node:child_process', () => ({ spawn: mock.spawn }))
vi.mock('./shell-snapshot', () => ({
  getOrCreateSnapshot: async () => null,
  wrapCommandWithSnapshot: (s: string) => s,
  shouldUseLoginShell: () => false,
}))
vi.mock('../../plugin-helpers', () => ({
  listEnabledPluginBinDirs: async () => [],
}))
vi.mock('../../utils/command-runtime', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../utils/command-runtime')>()),
  findBashExecutable: async () => 'bash',
  findPowerShellExecutable: async () => 'pwsh',
}))

describe('background task execution policy', () => {
  it('keeps Windows legacy PowerShell attached so it executes its command', async () => {
    const proc = Object.assign(new EventEmitter(), {
      stdout: new PassThrough(),
      stderr: new PassThrough(),
      unref: vi.fn(),
      pid: undefined,
    })
    mock.spawn.mockReturnValue(proc)
    vi.stubGlobal('process', { ...process, platform: 'win32' })
    try {
      await powerShellExecute({ command: 'Start-Sleep -Seconds 180', run_in_background: true })
      expect(mock.spawn.mock.lastCall?.[1]).toEqual([
        '-NoLogo',
        '-NoProfile',
        '-NonInteractive',
        '-ExecutionPolicy',
        'Bypass',
        '-Command',
        expect.stringMatching(/^Invoke-Expression \(\[System\.IO\.File\]::ReadAllText\('.+\.ps1'\)\)$/),
      ])
      expect(mock.spawn.mock.lastCall?.[1].join(' ')).not.toContain('Start-Sleep')
      expect(mock.spawn.mock.lastCall?.[2]).toMatchObject({ detached: false, windowsHide: true })
    } finally {
      vi.unstubAllGlobals()
    }
  })
  it.each([bashExecute, powerShellExecute])(
    'waits for full task output beyond the foreground timeout',
    async (execute) => {
      const signal = new AbortController().signal
      const onOutput = vi.fn()
      mock.foreground.mockResolvedValue({
        content: [{ type: 'text', text: 'complete output' }],
      })
      const deadline = Date.now() + 23 * 3600_000
      const result = await execute(
        { command: 'synthetic', run_in_background: true, timeout: 1 },
        { signal, taskDeadlineMs: deadline, onOutput, cwd: '/session project' },
      )
      expect(mock.foreground.mock.lastCall?.[2]).toBeGreaterThan(22 * 3600_000)
      expect(mock.foreground.mock.lastCall?.slice(5)).toEqual([signal, onOutput, '/session project'])
      // The command reaches the shell through a script file, never as argv.
      expect(mock.foreground.mock.lastCall?.[0]).not.toContain('synthetic')
      expect(result.content[0]?.text).toBe('complete output')
      await execute({ command: 'foreground', timeout: 99_000_000 })
      expect(mock.foreground.mock.lastCall?.[2]).toBe(1_800_000)
    },
  )
  it('keeps reading new output after the window passes its cap and reports dropped output', async () => {
    const proc = Object.assign(new EventEmitter(), {
      stdout: new PassThrough(),
      stderr: new PassThrough(),
      pid: 7654322,
      unref: vi.fn(),
      kill: vi.fn(),
    })
    mock.spawn.mockReturnValue(proc)
    await bashExecute({ command: 'noisy', run_in_background: true }, { tabId: 'cap-tab' })
    const id = localBackgroundShellSnapshot('cap-tab').at(-1)!.id.slice(6)
    const read = async () => (await readBackgroundOutput({ bash_id: id })).content[0]!.text
    const flush = () => new Promise((resolve) => setImmediate(resolve))

    // A two-byte character split between writes stays one character.
    const zh = Buffer.from('Ж', 'utf8')
    proc.stdout.write(zh.subarray(0, 1))
    proc.stdout.write(Buffer.concat([zh.subarray(1), Buffer.from('a'.repeat(60 * 1024))]))
    await flush()
    const first = await read()
    expect(first).toContain('Ж')
    expect(first).not.toContain('�')
    expect(first).not.toContain('dropped')

    // 150 K more characters: the first 60 K + 1 were read, the window keeps
    // 100 K, so 50 K of the new output fell out before this read.
    proc.stdout.write('b'.repeat(150 * 1024 - 8) + 'TAIL_ОК\n')
    await flush()
    const second = await read()
    expect(second).toContain('TAIL_ОК')
    expect(second).toMatch(/\[\d+ characters of stdout were dropped before this read/)
    proc.stdout.write('after-cap\n')
    await flush()
    const third = await read()
    expect(third).toContain('after-cap')
    expect(third).not.toContain('dropped')
    proc.emit('exit', 0)
  })

  it.each([bashExecute, powerShellExecute])(
    'keeps legacy background execution, cursors and owner-scoped stop',
    async (execute) => {
      const proc = Object.assign(new EventEmitter(), {
        stdout: new PassThrough(),
        stderr: new PassThrough(),
        pid: undefined,
        unref: vi.fn(),
        kill: vi.fn(),
      })
      // No real OS process: the synthetic PID only activates the legacy registry.
      Object.assign(proc, { pid: 7654321 })
      mock.spawn.mockReturnValue(proc)
      vi.spyOn(process, 'kill').mockReturnValue(true)
      const result = await execute(
        { command: 'legacy', run_in_background: true },
        { tabId: 'tab', shellOwner: 'owner', cwd: '/session project' },
      )
      expect(result.content[0]?.text).toContain('BashOutput')
      expect(mock.spawn.mock.lastCall?.[2].cwd).toBe('/session project')
      proc.stdout.write('still unread')
      const entry = backgroundShellSnapshot('tab', 'owner').at(-1)!
      expect(entry.output).toBe('still unread')
      expect(backgroundShellSnapshot('other-tab', 'owner')).toEqual([])
      expect(backgroundShellSnapshot('tab', 'replacement-owner')).toEqual([])
      expect(localBackgroundShellSnapshot('tab').some((s) => s.id === entry.id)).toBe(true)
      expect(localBackgroundShellSnapshot('other-tab')).toEqual([])
      await stopBackgroundShellForOwner('tab', 'other-owner', entry.id)
      expect(process.kill).not.toHaveBeenCalled()
      expect((await readBackgroundOutput({ bash_id: entry.id.slice(6) })).content[0]?.text).toContain('still unread')
      expect((await readBackgroundOutput({ bash_id: entry.id.slice(6) })).content[0]?.text).toContain('no new output')
      await stopBackgroundShellForOwner('tab', 'owner', entry.id)
      if (process.platform === 'win32') expect(proc.kill).toHaveBeenCalled()
      else expect(process.kill).toHaveBeenCalledWith(-7654321, 'SIGTERM')
      proc.emit('exit', 0)
      vi.restoreAllMocks()
    },
  )
})
