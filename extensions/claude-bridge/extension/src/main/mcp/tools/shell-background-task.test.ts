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
vi.mock('../../utils/command-runtime', () => ({
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
        '-NonInteractive',
        '-Command',
        expect.stringContaining(
          '[Console]::OutputEncoding = $OutputEncoding = New-Object System.Text.UTF8Encoding $false;\nStart-Sleep -Seconds 180',
        ),
      ])
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
      if (execute === powerShellExecute) {
        expect(mock.foreground.mock.lastCall?.[0]).toContain('[Console]::OutputEncoding = $OutputEncoding =')
        expect(mock.foreground.mock.lastCall?.[0]).toMatch(/\nsynthetic$/)
      }
      expect(result.content[0]?.text).toBe('complete output')
      await execute({ command: 'foreground', timeout: 99_000_000 })
      expect(mock.foreground.mock.lastCall?.[2]).toBe(1_800_000)
    },
  )
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
