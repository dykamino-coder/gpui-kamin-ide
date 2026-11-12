import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import { describe, expect, it } from 'vitest'
import { assertCmdLineFits, CMD_LINE_LIMIT, scriptInvocation, writeScriptFile } from './script-file'

describe('script files', () => {
  it('stores the script byte for byte: UTF-8 without BOM, line endings untouched', () => {
    const content = 'echo привет\r\necho ok\n'
    const file = writeScriptFile(content, '.sh')
    try {
      const bytes = fs.readFileSync(file.path)
      expect(bytes.subarray(0, 3).equals(Buffer.from([0xef, 0xbb, 0xbf]))).toBe(false)
      expect(bytes.equals(Buffer.from(content, 'utf8'))).toBe(true)
    } finally {
      file.dispose()
    }
    expect(fs.existsSync(file.path)).toBe(false)
    file.dispose()
  })

  it('passes only a short loader, never the script, to the shell', () => {
    const script = 'x'.repeat(100_000)
    const posix = scriptInvocation('bash', ['-lc'], script, 'win32')
    const ps = scriptInvocation('powershell', ['-NoProfile', '-Command'], script)
    try {
      expect(posix.args).toEqual(['-lc', posix.loader])
      expect(posix.loader).toMatch(/^\. '[^\\]+\.sh'$/)
      expect(ps.args).toEqual(['-NoProfile', '-Command', ps.loader])
      expect(ps.loader.length).toBeLessThan(500)
      expect(fs.readFileSync(ps.file.path, 'utf8')).toBe(`${script}\nif (-not $?) { exit 1 }\n`)
    } finally {
      posix.file.dispose()
      ps.file.dispose()
    }
  })
})

describe('cmd.exe line limit', () => {
  const comspec = 'C:\\WINDOWS\\system32\\cmd.exe'
  // Measured: cmd.exe accepts a 8191-character line and refuses one more.
  const fits = 'a'.repeat(CMD_LINE_LIMIT - `${comspec} /d /s /c ""`.length)

  it('accepts a line that fits and rejects a longer one before start', () => {
    expect(() => assertCmdLineFits(fits, [], 'win32', comspec)).not.toThrow()
    expect(() => assertCmdLineFits(`${fits}a`, [], 'win32', comspec)).toThrow(/too long for cmd\.exe.*not started/)
    expect(() => assertCmdLineFits('npx', ['-y', 'Ж'.repeat(9000)], 'win32', comspec)).toThrow(/not started/)
  })

  it('does not limit POSIX shells, whose limits fail loudly', () => {
    expect(() => assertCmdLineFits('x'.repeat(100_000), [], 'linux')).not.toThrow()
  })

  it.skipIf(process.platform !== 'win32')('matches the real cmd.exe boundary', () => {
    const real = process.env.ComSpec ?? 'cmd.exe'
    const room = CMD_LINE_LIMIT - `${real} /d /s /c ""`.length
    // A command of exactly `n` characters.
    const command = (n: number) => `echo ${'a'.repeat(n - 19)} >NUL & echo T`
    const ok = spawnSync(command(room), { shell: true, encoding: 'utf8' })
    expect(ok.stdout).toContain('T')
    expect(() => assertCmdLineFits(command(room), [], 'win32', real)).not.toThrow()
    const over = spawnSync(command(room + 1), { shell: true, encoding: 'utf8' })
    expect(over.stdout).not.toContain('T')
    expect(() => assertCmdLineFits(command(room + 1), [], 'win32', real)).toThrow(/not started/)
  })
})
