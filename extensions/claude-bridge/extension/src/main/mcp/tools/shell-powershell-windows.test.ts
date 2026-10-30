import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, expect, it, vi } from 'vitest'

vi.mock('../../plugin-helpers', () => ({ listEnabledPluginBinDirs: async () => [] }))
const fixture = vi.hoisted(() => ({ executable: '' }))
vi.mock('../../utils/command-runtime', () => ({ findPowerShellExecutable: async () => fixture.executable }))
const directories: string[] = []
afterEach(() => {
  for (const directory of directories.splice(0)) fs.rmSync(directory, { recursive: true })
  vi.resetModules()
})
const engines = [
  ['Windows PowerShell 5.1', 'C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe'],
  ['PowerShell 7', 'C:\\Program Files\\PowerShell\\7\\pwsh.exe'],
]

for (const [name, executable] of engines) {
  it.skipIf(process.platform !== 'win32' || !fs.existsSync(executable))(
    `${name}: UTF-8 cmdlets, native commands, git and stderr in foreground and background`,
    async () => {
      fixture.executable = executable
      const { powerShellExecute, localBackgroundShellSnapshot, readBackgroundOutput } = await import('./shell')
      const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-powershell-utf8-'))
      directories.push(directory)
      const git = (args: string[]) => execFileSync('git', args, { cwd: directory, windowsHide: true })
      git(['init', '--quiet'])
      git([
        '-c',
        'user.name=Bridge Test',
        '-c',
        'user.email=bridge@example.invalid',
        'commit',
        '--allow-empty',
        '-qm',
        'Русский коммит',
      ])
      const command =
        "Write-Output 'Привет'; cmd /d /c echo привет; git --no-pager log -1 --format=%s; [Console]::Error.WriteLine('Ошибка'); Write-Error 'Сбой' -ErrorAction Continue"
      const check = (text: string) => {
        for (const expected of ['Привет', 'привет', 'Русский коммит', 'Ошибка', 'Сбой'])
          expect(text).toContain(expected)
        expect(text).not.toContain('\ufffd')
      }
      const streamed: string[] = []
      const result = await powerShellExecute({ command }, { cwd: directory, onOutput: (s) => streamed.push(s) })
      check(result.content[0].text)
      check(streamed.join(''))
      await powerShellExecute({ command, run_in_background: true }, { cwd: directory, tabId: name })
      await vi.waitFor(() => expect(localBackgroundShellSnapshot(name)[0]?.running).toBe(false), { timeout: 15000 })
      const shell = localBackgroundShellSnapshot(name)[0]
      expect(shell.command).toBe(command)
      check(shell.output)
      check((await readBackgroundOutput({ bash_id: shell.id.slice(6) })).content[0].text)

      // Prefixing must preserve explicit exit, native LASTEXITCODE and terminating errors.
      const exit = await powerShellExecute({
        command: 'cmd /d /c exit 7; Write-Output $LASTEXITCODE; exit $LASTEXITCODE',
      })
      expect(exit.content[0].text).toMatch(/7[\s\S]*\[Exit code: 7\]/)
      await powerShellExecute(
        { command: 'cmd /d /c exit 7; Write-Output $LASTEXITCODE; exit $LASTEXITCODE', run_in_background: true },
        { tabId: name },
      )
      await vi.waitFor(() => expect(localBackgroundShellSnapshot(name).at(-1)?.running).toBe(false), {
        timeout: 15000,
      })
      const exited = localBackgroundShellSnapshot(name).at(-1)!
      const status = (await readBackgroundOutput({ bash_id: exited.id.slice(6) })).content[0].text
      expect(status).toMatch(/exited \(code 7\)[\s\S]*7/)
      const failure = await powerShellExecute({ command: "throw 'Ошибка'; Write-Output 'unreachable'" })
      expect(failure.content[0].text).toContain('Ошибка')
      expect(failure.content[0].text).toContain('[Exit code: 1]')
      expect(failure.content[0].text).not.toContain('unreachable\r\n')
    },
    30000,
  )
}
