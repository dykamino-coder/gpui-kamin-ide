import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'

vi.mock('./ripgrep-path', async (importOriginal) => {
  const original = await importOriginal<typeof import('./ripgrep-path')>()
  return { ...original, ripgrepPath: vi.fn(original.ripgrepPath) }
})
import { ripgrepPath } from './ripgrep-path'
import { globQuery, listGlob, newestFirst, ripgrepListArgs } from './glob-tool'
import { globSearch } from './file-system'

const text = (r: { content: Array<{ text?: string }> }): string => r.content.map((c) => c.text ?? '').join('')

describe('Glob pattern meaning', () => {
  it('keeps a relative pattern and skips VCS and dependency folders', () => {
    const q = globQuery('src/**/*.ts', '/repo', 'linux')
    expect(q).toEqual({ base: '/repo', pattern: 'src/**/*.ts', skipped: ['.git', '.svn', '.hg', '.bzr', '.jj', '.sl', 'node_modules'] })
    expect(ripgrepListArgs(q).slice(0, 8)).toEqual(['--files', '--null', '--hidden', '--no-ignore', '--no-config', '--follow', '--glob', 'src/**/*.ts'])
    expect(ripgrepListArgs(q).slice(-2)).toEqual(['--', '/repo'])
  })

  it('moves the literal part of an absolute or parent pattern into the base and anchors the rest', () => {
    expect(globQuery('/work/app/src/*.ts', '/repo', 'linux')).toMatchObject({ base: path.resolve('/work/app/src'), pattern: '/*.ts' })
    expect(globQuery('../other/**/*.rs', '/repo/a', 'linux')).toMatchObject({ base: path.resolve('/repo/other'), pattern: '/**/*.rs' })
    expect(globQuery('./x/*.md', '/repo', 'linux').pattern).toBe('x/*.md')
  })

  it('reads Windows separators and drive roots', () => {
    const q = globQuery('C:\\work\\app\\*.ts', 'C:\\repo', 'win32')
    expect(q.pattern).toBe('/*.ts')
    expect(q.base.replace(/\\/g, '/')).toMatch(/work\/app$/)
  })

  it('lists node_modules or .git only when the pattern names them', () => {
    expect(globQuery('node_modules/react/*.json', '/repo', 'linux').skipped).not.toContain('node_modules')
    expect(globQuery('**/*.json', '/repo/node_modules', 'linux').skipped).not.toContain('node_modules')
    expect(globQuery('.git/HEAD', '/repo', 'linux').skipped).not.toContain('.git')
  })
})

let dir = ''
beforeAll(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'glob-tool-'))
  const put = (rel: string, ageSeconds: number) => {
    const p = path.join(dir, rel)
    fs.mkdirSync(path.dirname(p), { recursive: true })
    fs.writeFileSync(p, rel)
    const t = Date.now() / 1000 - ageSeconds
    fs.utimesSync(p, t, t)
  }
  put('top.ts', 50)
  put('src/a.ts', 40)
  put('src/deep/b.ts', 10)
  put('src/deep/c.md', 5)
  put('.hidden/d.ts', 30)
  put('dist/e.ts', 20)
  put('node_modules/pkg/f.ts', 1)
  put('.git/g.ts', 1)
  put('bin/[x].ts', 60)
})
afterAll(() => fs.rmSync(dir, { recursive: true, force: true }))

const rel = (files: string[]): string[] => files.map((f) => path.relative(dir, f).replace(/\\/g, '/'))

for (const engine of ['ripgrep', 'fast-glob'] as const) {
  describe(`Glob listing with ${engine}`, () => {
    beforeAll(() => {
      if (engine === 'fast-glob') vi.mocked(ripgrepPath).mockResolvedValue(null)
    })
    afterAll(async () => {
      const actual = await vi.importActual<typeof import('./ripgrep-path')>('./ripgrep-path')
      vi.mocked(ripgrepPath).mockImplementation(actual.ripgrepPath)
    })

    const list = async (pattern: string, base = dir) => {
      if (engine === 'ripgrep' && !(await ripgrepPath())) return null
      return rel(await newestFirst((await listGlob(globQuery(pattern, base))).files))
    }

    it('matches a slashless pattern at any depth, newest first, with hidden and build folders', async () => {
      const out = await list('*.ts')
      if (!out) return
      expect(out).toEqual(['src/deep/b.ts', 'dist/e.ts', '.hidden/d.ts', 'src/a.ts', 'top.ts', 'bin/[x].ts'])
    })

    it('anchors a pattern with a folder and an absolute pattern to their base', async () => {
      const a = await list('src/*.ts')
      if (!a) return
      expect(a).toEqual(['src/a.ts'])
      expect(await list(`${dir.replace(/\\/g, '/')}/src/deep/*.ts`)).toEqual(['src/deep/b.ts'])
      expect(await list('**/*.{md,ts}', path.join(dir, 'src'))).toEqual(['src/deep/c.md', 'src/deep/b.ts', 'src/a.ts'])
    })

    it('lists node_modules when the pattern names it', async () => {
      const out = await list('node_modules/**/*.ts')
      if (!out) return
      expect(out).toEqual(['node_modules/pkg/f.ts'])
    })

    it('stops at the listing cap', async () => {
      if (engine === 'ripgrep' && !(await ripgrepPath())) return
      const r = await listGlob(globQuery('**/*.ts', dir), 2)
      expect(r.capped).toBe(true)
      expect(r.files).toHaveLength(2)
    })
  })
}

describe('Glob tool', () => {
  it('reports a missing folder instead of an empty match', async () => {
    const r = await globSearch({ pattern: '*.ts', path: path.join(dir, 'nope') })
    expect(r.isError).toBe(true)
    expect(text(r)).toContain('does not exist')
  })
})
