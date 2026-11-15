import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, expect, it, vi } from 'vitest'

vi.mock('../../plugin-helpers', () => ({ listEnabledPluginBinDirs: async () => [] }))
const fixture = vi.hoisted(() => ({ executable: '' }))
vi.mock('../../utils/command-runtime', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../utils/command-runtime')>()),
  findPowerShellExecutable: async () => fixture.executable,
}))
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
      expect(exit.content[0].text).toBe('Exit code 7\n7')
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
      expect(failure.content[0].text).toMatch(/^Exit code 1\n/)
      expect(failure.content[0].text).not.toContain('unreachable\r\n')
    },
    30000,
  )

  it.skipIf(process.platform !== 'win32' || !fs.existsSync(executable))(
    `${name}: buffered output keeps every line, one table header, live streaming and host order`,
    async () => {
      fixture.executable = executable
      const { powerShellExecute } = await import('./shell')
      const many = await powerShellExecute({ command: '1..10000' })
      const lines = many.content[0].text.split(/\r?\n/).filter(Boolean)
      expect(lines).toHaveLength(10000)
      expect(lines.at(-1)).toBe('10000')

      // One formatter for the whole output: a table split across flushes keeps one header.
      const table = await powerShellExecute({
        command: "1..400 | ForEach-Object { [pscustomobject]@{ Index = $_; Name = 'n' + $_ } }",
      })
      expect(table.content[0].text.match(/Index Name/g)).toHaveLength(1)

      // Output written before a pause reaches the live stream before the command ends.
      const stamps: Array<[string, number]> = []
      const started = Date.now()
      const slow = await powerShellExecute(
        { command: "Write-Output 'first'; Start-Sleep -Milliseconds 1500; Write-Output 'second'" },
        { onOutput: (s) => stamps.push([s, Date.now() - started]) },
      )
      const first = stamps.find(([s]) => s.includes('first'))
      const second = stamps.find(([s]) => s.includes('second'))
      expect(first).toBeDefined()
      expect(second![1] - first![1]).toBeGreaterThan(1000)
      expect(slow.content[0].text).toMatch(/first\r?\nsecond/)

      // Host output and pipeline output keep their order.
      const mixed = await powerShellExecute({ command: "1..3 | ForEach-Object { $_; Write-Host ('h' + $_) }" })
      expect(mixed.content[0].text.replace(/\r/g, '')).toContain('1\nh1\n2\nh2\n3\nh3')

      // [Environment]::Exit() right after the output still delivers all of it.
      const exited = await powerShellExecute({ command: '1..2000; [Environment]::Exit(4)' })
      expect(exited.exitCode).toBe(4)
      expect(exited.content[0].text).toBe('Exit code 4\n' + Array.from({ length: 2000 }, (_, i) => i + 1).join('\r\n'))
      const quick = await powerShellExecute({ command: '1..3; [Environment]::Exit(0)' })
      expect(quick.content[0].text).toBe('1\r\n2\r\n3')

      // stderr and stdout share one stream in their order, as in native PowerShell.
      const ordered = await powerShellExecute({
        command: "[Console]::Error.WriteLine('e1'); Write-Output 'o1'; [Console]::Error.WriteLine('e2'); exit 2",
      })
      expect(ordered.content[0].text).toBe('Exit code 2\ne1\r\no1\r\ne2')
      expect((await powerShellExecute({ command: '$null' })).content[0].text).toBe('(PowerShell completed with no output)')
      if (name === 'PowerShell 7') {
        // The host's own error view, byte for byte as native PowerShell 2.1.322 returns it.
        expect((await powerShellExecute({ command: "throw 'boom'" })).content[0].text).toBe('Exit code 1\nException: boom')
        expect((await powerShellExecute({ command: "Write-Output before\nthrow \"boom two\"" })).content[0].text).toBe(
          'Exit code 1\nbefore\r\nException: \r\nLine |\r\n   2 |  throw "boom two"\r\n     |  ~~~~~~~~~~~~~~~~\r\n     | boom two',
        )
      }
    },
    60000,
  )
}
