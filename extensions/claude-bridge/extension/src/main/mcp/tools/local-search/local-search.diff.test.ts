// Differential test of the in-process Glob and Grep against the real ripgrep:
// every search the in-process path answers must equal ripgrep's output byte
// for byte (Grep: each file's block of stdout and the exit code; Glob: the
// listed file set, which the tool sorts newest first). A fixed tree
// covers ignore files, hidden and binary files, encodings, CRLF and long
// lines; random trees and patterns follow, with the seed in the log
// (LOCAL_SEARCH_SEED=<n> repeats a run). Without ripgrep the test is skipped
// locally and fails in CI.
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { ripgrepPath } from '../ripgrep-path'
import { ripgrepArgs, grepOptions } from '../grep-tool'
import { globQuery, ripgrepListArgs } from '../glob-tool'
import { localGrep } from './local-grep'
import { localGlobFiles } from './local-glob'

const VCS = ['.git', '.svn', '.hg', '.bzr', '.jj', '.sl']
const unlimited = () => ({ entries: 1e9, bytes: 1e12, deadline: Infinity })
let rg: string | null = null
const savedXdg = process.env.XDG_CONFIG_HOME
let tmp = ''

beforeAll(async () => {
  rg = process.env.KAMIN_RG_PATH || (await ripgrepPath())
  if (!rg && process.env.CI) throw new Error('ripgrep is required for the differential test in CI')
  tmp = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'local-search-')))
  // The machine's own global git excludes would decide some cases; an empty
  // config home makes the fixed cases the same everywhere.
  process.env.XDG_CONFIG_HOME = path.join(tmp, 'empty-config')
})
afterAll(() => {
  if (savedXdg === undefined) delete process.env.XDG_CONFIG_HOME
  else process.env.XDG_CONFIG_HOME = savedXdg
  fs.rmSync(tmp, { recursive: true, force: true })
})

function put(root: string, rel: string, data: string | Buffer): void {
  const p = path.join(root, rel)
  fs.mkdirSync(path.dirname(p), { recursive: true })
  fs.writeFileSync(p, data)
}

interface Counts {
  local: number
  delegated: number
}

/** Compares one Grep call; returns whether the in-process path answered. */
function checkGrep(input: Record<string, unknown>, label: string): boolean {
  const o = grepOptions(input)
  if (typeof o === 'string') throw new Error(o)
  const local = localGrep({ ...o, vcsDirectories: VCS, maxColumns: 500 }, unlimited())
  if (!local) return false
  const run = spawnSync(rg!, ripgrepArgs(o), { encoding: 'buffer' })
  expect({ label, stdout: byFile(local.stdout), code: local.code }).toEqual({ label, stdout: byFile(run.stdout.toString('utf8')), code: run.status })
  return true
}

/** ripgrep prints whole per-file blocks in a walk order of its own (parallel
 *  workers; the single-threaded order also differs between versions): blocks
 *  are compared byte for byte, the order of files is not. A stable sort by
 *  the file name keeps each file's lines in their printed order. */
function byFile(stdout: string): string {
  const lines = stdout.split('\n')
  const key = (l: string): string => {
    const i = l.indexOf(':', /^[A-Za-z]:[\\/]/.test(l) ? 2 : 0)
    return i === -1 ? l : l.slice(0, i)
  }
  return lines.map((l, i) => [key(l), i, l] as const).sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : a[1] - b[1])).map((x) => x[2]).join('\n')
}

function checkGlob(pattern: string, base: string, label: string): boolean {
  const q = globQuery(pattern, base)
  const local = localGlobFiles(q, unlimited())
  if (!local) return false
  const run = spawnSync(rg!, ripgrepListArgs(q), { cwd: q.base, encoding: 'buffer' })
  const listed = run.stdout.toString('utf8').split('\0').filter(Boolean).map((p) => p.replace(/\\/g, '/'))
  expect({ label, files: [...local].sort() }).toEqual({ label, files: listed.sort() })
  return true
}

function fixedTree(root: string, repo: boolean): void {
  if (repo) put(root, '.git/info/exclude', '# comment\nexcluded.txt\n')
  put(root, '.gitignore', '*.log\nbuild/\n!keep.log\n/rooted.txt\ndocs/**/*.tmp\n')
  put(root, 'src/.gitignore', 'gen-*\n!gen-keep.ts\n')
  put(root, '.ignore', 'ignored-by-dot-ignore.txt\n')
  put(root, 'sub/.rgignore', '!*.log\n')
  put(root, 'src/a.ts', 'export function alpha() {}\nconst x = 1\n')
  put(root, 'src/gen-1.ts', 'export function generated() {}\n')
  put(root, 'src/gen-keep.ts', 'export function kept() {}\n')
  put(root, 'src/deep/b.tsx', 'export function beta() {}\r\nexport const crlf = 2\r\n')
  put(root, 'app.log', 'export function logged() {}\n')
  put(root, 'keep.log', 'export function keptLog() {}\n')
  put(root, 'sub/x.log', 'export function subLog() {}\n')
  put(root, 'rooted.txt', 'export function rooted() {}\n')
  put(root, 'sub/rooted.txt', 'export function notRooted() {}\n')
  put(root, 'build/out.js', 'export function built() {}\n')
  put(root, 'docs/a/b/c.tmp', 'export function tmp() {}\n')
  put(root, 'excluded.txt', 'export function excluded() {}\n')
  put(root, 'ignored-by-dot-ignore.txt', 'export function dotIgnore() {}\n')
  put(root, '.hidden/h.ts', 'export function hidden() {}\n')
  put(root, '.env', 'export FOO=1\n')
  put(root, 'node_modules/pkg/i.js', 'export function dep() {}\n')
  put(root, 'unicode.txt', 'строка один\nслово_2 ✓ export\nnaïve café\n')
  put(root, 'nonl.txt', 'export function noNewline() {}')
  put(root, 'empty.txt', '')
  put(root, 'bin/blob.bin', Buffer.from([0x65, 0x78, 0x70, 0x6f, 0x72, 0x74, 0, 1, 2, 0x0a]))
  put(root, 'bin/blob2.bin', Buffer.from([0x7a, 0x7a, 0, 1, 2, 0x0a]))
  put(root, 'enc/utf16.txt', Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from('export function u16() {}\n', 'utf16le')]))
  put(root, 'enc/bom8.txt', Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from('export function bom() {}\n')]))
  put(root, 'enc/latin1.txt', Buffer.from([0x63, 0x61, 0x66, 0xe9, 0x0a, 0x7a, 0x0a]))
  put(root, 'long/n500.txt', 'x'.repeat(500))
  put(root, 'long/n499.txt', 'x'.repeat(499))
  put(root, 'long/l.txt', ['x'.repeat(499), 'x'.repeat(500), 'x'.repeat(501), 'я'.repeat(250), 'я'.repeat(251)].join('\n') + '\n')
}

const GREP_PATTERNS = ['export function', 'export', 'function \\w+', '^export', '\\)$', 'x+$', 'я+', '\\bfunction\\b', '\\d', 'a|b', 'fun.tion', '[a-c]', '[^a-z]', '\\s\\{', 'ze?ro', '(al|be)ta', 'e{2,}', 'zz', 'caf', 'строка', '\\w+_\\d', 'ü', '\\Bunction', '^$', '^\\s*$', '(^a|b$)', '^(export|const)']

for (const repo of [true, false]) {
  describe(`in-process search equals ripgrep (${repo ? 'git repository' : 'plain folder'})`, () => {
    let root = ''
    beforeAll(() => {
      root = path.join(tmp, repo ? 'repo' : 'plain')
      fixedTree(root, repo)
    })

    it('answers Grep exactly as ripgrep', () => {
      if (!rg) return
      const c: Counts = { local: 0, delegated: 0 }
      for (const pattern of GREP_PATTERNS) {
        for (const mode of ['content', 'files_with_matches', 'count']) {
          for (const extra of [{}, { '-i': true }, { '-n': false }, { glob: '*.ts' }, { glob: '!*.ts' }, { glob: '*.{ts,tsx}' }, { path: path.join(root, 'src') }, { path: path.join(root, 'unicode.txt') }, { path: path.join(root, 'app.log') }]) {
            const input = { pattern, output_mode: mode, path: root, ...extra }
            if (checkGrep(input, JSON.stringify(input))) c.local++
            else c.delegated++
          }
        }
      }
      // The common shapes must stay in-process, or the test proves nothing.
      expect(c.local).toBeGreaterThan(c.delegated)
    }, 120_000)

    it('answers Glob exactly as ripgrep', () => {
      if (!rg) return
      let local = 0
      for (const pattern of ['*.ts', '**/*.ts', 'src/*.ts', 'src/**/*.{ts,tsx}', '*.log', '**/*', '.*', '*', 'node_modules/**/*.js', '**/deep/*', 'src', 'b?n/*.bin', '[a-e]*.txt', '[!a-e]*.txt', '/enc/*', `${root.replace(/\\/g, '/')}/src/*.ts`]) {
        if (checkGlob(pattern, root, pattern)) local++
      }
      expect(local).toBeGreaterThan(12)
    }, 120_000)
  })
}

describe('in-process search equals ripgrep with a global git excludes file', () => {
  let saved: string | undefined
  beforeAll(() => {
    saved = process.env.XDG_CONFIG_HOME
  })
  afterAll(() => {
    process.env.XDG_CONFIG_HOME = saved
  })
  it('applies unanchored lines everywhere and anchored ones under the working directory only', () => {
    if (!rg) return
    const xdg = path.join(tmp, 'xdg')
    put(xdg, 'git/ignore', 'sub/x.txt\n/top.txt\n*.zz\nskipdir/\n')
    process.env.XDG_CONFIG_HOME = xdg
    const root = path.join(tmp, 'global-repo')
    fs.mkdirSync(path.join(root, '.git'), { recursive: true })
    for (const f of ['top.txt', 'a.zz', 'sub/x.txt', 'sub/top.txt', 'skipdir/y.txt', 'keep.txt']) put(root, f, 'hit\n')
    let local = 0
    for (const mode of ['content', 'files_with_matches']) if (checkGrep({ pattern: 'hit', output_mode: mode, path: root }, `global ${mode}`)) local++
    expect(local).toBe(2)
  }, 60_000)
})

/** Small deterministic PRNG (mulberry32). */
function prng(seed: number): () => number {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

describe('in-process search equals ripgrep on random trees', () => {
  const seed = Number(process.env.LOCAL_SEARCH_SEED) || Date.now() % 1_000_000_007
  it(`random trees and patterns (seed ${seed})`, () => {
    if (!rg) return
    console.log(`local-search differential seed: ${seed}`)
    const r = prng(seed)
    const pick = <T,>(xs: readonly T[]): T => xs[Math.floor(r() * xs.length)]!
    const names = ['a', 'b', 'ab', 'x.ts', 'y.tsx', 'z.js', '.dot', 'Ä.txt', 'gen-1.ts', 'n m.md', 'c.log', 'd', 'build', 'src']
    const words = ['export', 'function', 'const', 'alpha', 'Beta', '42', 'слово', 'x_1', '(', ')', '{', '}', '-', '  ', 'café', 'ß', 'end$']
    const globAtoms = ['*.ts', '*.log', 'build/', '/a', 'src/**', '!*.log', '**/b', 'a*', '?.ts', '*.{ts,js}', '[ab]*', '!gen-*', 'd/', '# c', 'x.ts']
    const regexAtoms = ['export', 'a', 'B', '\\w', '\\d+', '.', '\\s', '[a-f]', '[^ ]', 'é', '\\b', 'x_\\d', '(a|b)', 'c?', 'o{1,2}', '^', '$', 'ß', '\\(']
    let local = 0
    for (let t = 0; t < 40; t++) {
      const root = path.join(tmp, `rand-${t}`)
      const dirs = ['']
      for (let i = 0; i < 1 + r() * 4; i++) dirs.push(path.join(pick(dirs), pick(names)))
      for (let i = 0; i < 4 + r() * 12; i++) {
        const lines = Array.from({ length: Math.floor(r() * 5) }, () => Array.from({ length: 1 + Math.floor(r() * 5) }, () => pick(words)).join(r() < 0.5 ? ' ' : ''))
        try {
          put(root, path.join(pick(dirs), pick(names) + (r() < 0.3 ? '.ts' : '')), lines.join(r() < 0.2 ? '\r\n' : '\n') + (r() < 0.8 ? '\n' : ''))
        } catch {
          // a name taken by a directory
        }
      }
      if (r() < 0.6) fs.mkdirSync(path.join(root, '.git'), { recursive: true })
      for (const d of dirs) {
        if (r() < 0.4) {
          try {
            put(root, path.join(d, pick(['.gitignore', '.ignore', '.rgignore'])), Array.from({ length: 1 + Math.floor(r() * 3) }, () => pick(globAtoms)).join('\n') + '\n')
          } catch {
            // a file in the way
          }
        }
      }
      for (let q = 0; q < 12; q++) {
        const pattern = Array.from({ length: 1 + Math.floor(r() * 3) }, () => pick(regexAtoms)).join('')
        const input: Record<string, unknown> = { pattern, output_mode: pick(['content', 'files_with_matches', 'count']), path: root }
        if (r() < 0.3) input['-i'] = true
        if (r() < 0.3) input.glob = pick(['*.ts', '!*.log', '*.{ts,js}', 'x*'])
        if (checkGrep(input, `seed ${seed} tree ${t} ${JSON.stringify(input)}`)) local++
        const g = pick(['*.ts', '**/*', '**/a/*', 'src/**/*.ts', '*.{ts,tsx}', '?.ts', '[a-c]*', 'b*/**', '.*'])
        checkGlob(g, root, `seed ${seed} tree ${t} glob ${g}`)
      }
    }
    expect(local).toBeGreaterThan(100)
  }, 300_000)
})
