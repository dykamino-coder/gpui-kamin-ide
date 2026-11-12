import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./ripgrep-path', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./ripgrep-path')>()),
  ripgrepPath: vi.fn(async () => null),
}))
import { findRipgrep, ripgrepPath } from './ripgrep-path'
import { grepOptions, grepSearch, ripgrepArgs, splitGlobs } from './grep-tool'

const text = (r: { content: Array<{ text?: string }> }): string => r.content.map((c) => c.text ?? '').join('')

describe('ripgrep arguments (native Grep parity)', () => {
  it('searches hidden files, skips VCS dirs, caps long lines and passes every flag', () => {
    const o = grepOptions(
      { pattern: '-foo', path: '/repo', output_mode: 'content', glob: '*.{ts,tsx} *.rs,!gen/**', type: 'ts', '-i': true, multiline: true, '-C': 2 },
      '/repo',
    )
    if (typeof o === 'string') throw new Error(o)
    const args = ripgrepArgs(o)
    expect(args.slice(0, 3)).toEqual(['--hidden', '--max-columns', '500'])
    expect(args).toContain('!.git')
    expect(args.join(' ')).toContain('-i -n -U --multiline-dotall -C 2 --glob *.{ts,tsx} --glob *.rs --glob !gen/** --type ts')
    expect(args.slice(-4)).toEqual(['-e', '-foo', '--', o.searchPath])
  })

  it('splits globs on whitespace and commas outside braces', () => {
    expect(splitGlobs('*.{ts,tsx}  a.js,b.js')).toEqual(['*.{ts,tsx}', 'a.js', 'b.js'])
  })

  it('rejects an unknown output_mode instead of guessing', () => {
    expect(grepOptions({ pattern: 'x', output_mode: 'lines' })).toMatch(/output_mode must be/)
  })
})

describe('ripgrep resolution', () => {
  const probe = (files: string[], which: string | null) => ({
    platform: 'win32' as const,
    env: {},
    execPath: 'C:\\K\\runtime\\kaminhost.exe',
    exists: (p: string) => files.includes(p),
    which: async () => which,
  })
  it('prefers the bundled binary next to the job runner, then PATH', async () => {
    expect(await findRipgrep(probe([path.join('C:\\K\\runtime', '..', 'rg.exe')], 'C:\\bin\\rg.exe'))).toBe(path.join('C:\\K\\runtime', '..', 'rg.exe'))
    expect(await findRipgrep(probe([], 'C:\\bin\\rg.exe'))).toBe('C:\\bin\\rg.exe')
    expect(await findRipgrep(probe([], null))).toBeNull()
  })
})

describe('fallback without ripgrep', () => {
  let dir = ''
  beforeEach(() => {
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'grep-fb-'))
    fs.mkdirSync(path.join(dir, 'src'))
    fs.mkdirSync(path.join(dir, '.github'))
    fs.writeFileSync(path.join(dir, 'src', 'a.ts'), 'needle one\nother\n')
    fs.writeFileSync(path.join(dir, 'src', 'b.rs'), 'needle two\n')
    fs.writeFileSync(path.join(dir, '.github', 'ci.yml'), 'needle three\n')
    fs.writeFileSync(path.join(dir, 'blob.bin'), Buffer.from([0x6e, 0x65, 0x65, 0x64, 0x6c, 0x65, 0]))
  })
  afterEach(() => fs.rmSync(dir, { recursive: true, force: true }))

  it('refuses arguments it cannot honour instead of ignoring them', async () => {
    expect(vi.mocked(ripgrepPath)).toBeDefined()
    for (const extra of [{ type: 'ts' }, { multiline: true }, { output_mode: 'content', '-C': 1 }]) {
      const r = await grepSearch({ pattern: 'needle', path: dir, ...extra })
      expect(r.isError).toBe(true)
      expect(text(r)).toContain('does not support')
    }
  })

  it('applies glob like ripgrep, searches hidden files, skips binaries and says it is a fallback', async () => {
    const all = text(await grepSearch({ pattern: 'needle', path: dir }))
    expect(all).toContain(path.join(dir, '.github', 'ci.yml'))
    expect(all).not.toContain('blob.bin')
    expect(all).toContain('ripgrep is not installed')
    const ts = text(await grepSearch({ pattern: 'needle', path: dir, glob: '*.ts' }))
    expect(ts).toContain(path.join(dir, 'src', 'a.ts'))
    expect(ts).not.toContain('b.rs')
    const content = text(await grepSearch({ pattern: 'needle', path: dir, glob: '*.rs', output_mode: 'content' }))
    expect(content).toContain(`${path.join(dir, 'src', 'b.rs')}:1:needle two`)
  })

  it('reports a missing path as an error', async () => {
    const r = await grepSearch({ pattern: 'x', path: path.join(dir, 'missing') })
    expect(r.isError).toBe(true)
  })
})
