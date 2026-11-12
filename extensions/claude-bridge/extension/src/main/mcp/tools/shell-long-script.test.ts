// Scripts longer than any command-line limit run completely and exactly.
// Git Bash used to receive the script as one `-c` argument and silently cut
// it after about 8 K characters.
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { findBashExecutable, findPowerShellExecutable } from '../../utils/command-runtime'
import { scriptDirectory } from '../../utils/script-file'

vi.mock('../../plugin-helpers', () => ({ listEnabledPluginBinDirs: async () => [] }))

const { bashExecute, powerShellExecute, localBackgroundShellSnapshot, readBackgroundOutput } = await import('./shell')

let bash: string | null = null
try {
  bash = await findBashExecutable()
} catch {
  bash = null
}
let pwsh: string | null = null
try {
  pwsh = await findPowerShellExecutable()
} catch {
  pwsh = null
}

const ownScripts = (): string[] => {
  try {
    return fs.readdirSync(scriptDirectory()).filter((name) => name.startsWith(`cmd-${process.pid}-`))
  } catch {
    return []
  }
}
const text = (result: { content: Array<{ text: string }> }): string => result.content.map((c) => c.text).join('')

// 70 000 Cyrillic characters: ~137 KB of UTF-8, beyond every argv limit.
const BODY = 'Ж'.repeat(70_000)
const TAIL = 'TAIL_MARKER_ХВОСТ'

afterEach(() => {
  vi.restoreAllMocks()
})

describe.skipIf(!bash)('Bash: long scripts', () => {
  it('runs a 137 KB heredoc script to its last line with the right status', async () => {
    const command = [
      "cat <<'EOF' > /dev/null",
      BODY,
      'EOF',
      "count=$(cat <<'EOF' | wc -c",
      BODY,
      'EOF',
      ')',
      'echo "chars=$count"',
      `echo ${TAIL}`,
      'exit 3',
    ].join('\n')
    const result = await bashExecute({ command, timeout: 60_000 })
    const out = text(result)
    // Bytes, independent of the locale: 2 per character plus the final newline.
    expect(out).toMatch(/chars=\s*140001/)
    expect(out).toContain(TAIL)
    expect(result.exitCode).toBe(3)
    expect(ownScripts()).toEqual([])
  }, 90_000)

  it('keeps -c semantics: $0, no positional arguments, set -e, stdin', async () => {
    const reference = spawnSync(bash!, ['-c', 'echo "$0"'], { encoding: 'utf8' }).stdout.trim()
    const result = await bashExecute({
      command: [
        'echo "zero=$0 argc=$#"',
        'read -r line || echo "stdin=empty"',
        'set -e',
        'false',
        'echo unreachable',
      ].join('\n'),
    })
    const out = text(result)
    expect(out).toContain(`zero=${reference} argc=0`)
    expect(out).toContain('stdin=empty')
    expect(out).not.toContain('unreachable')
    expect(result.exitCode).toBe(1)
  }, 60_000)

  it('runs a long background script completely and removes its file', async () => {
    const tab = `long-bg-${process.pid}`
    await bashExecute(
      { command: `: '${BODY}'\necho ${TAIL}`, run_in_background: true },
      { tabId: tab },
    )
    await vi.waitFor(() => expect(localBackgroundShellSnapshot(tab).at(-1)?.running).toBe(false), {
      timeout: 30_000,
    })
    const shell = localBackgroundShellSnapshot(tab).at(-1)!
    const status = text(await readBackgroundOutput({ bash_id: shell.id.slice(6) }))
    expect(status).toContain('exited (code 0)')
    expect(status).toContain(TAIL)
    await vi.waitFor(() => expect(ownScripts()).toEqual([]), { timeout: 5_000 })
  }, 60_000)

  it('removes the script file when the command times out', async () => {
    const result = await bashExecute({ command: `: '${BODY}'\nsleep 30\necho ${TAIL}`, timeout: 1500 })
    expect(text(result)).toContain('timed out')
    expect(text(result)).not.toContain(TAIL)
    await vi.waitFor(() => expect(ownScripts()).toEqual([]), { timeout: 5_000 })
  }, 60_000)
})

describe.skipIf(!pwsh)('PowerShell: long scripts', () => {
  it('runs a 137 KB script to its last line with -Command exit semantics', async () => {
    const command = `$s = @'\n${BODY}\n'@\nWrite-Output "chars=$($s.Length)"\nWrite-Output '${TAIL}'\nexit 4`
    const result = await powerShellExecute({ command, timeout: 60_000 })
    expect(text(result)).toContain('chars=70000')
    expect(text(result)).toContain(TAIL)
    expect(result.exitCode).toBe(4)
    expect(ownScripts()).toEqual([])
  }, 90_000)

  it('reports a failed last statement as exit code 1, as -Command does', async () => {
    const result = await powerShellExecute({ command: "Write-Error 'сбой'" })
    expect(result.exitCode).toBe(1)
    const ok = await powerShellExecute({ command: "Write-Error 'сбой'\nWrite-Output 'после'" })
    expect(ok.exitCode).toBe(0)
    expect(text(ok)).toContain('после')
  }, 60_000)
})
