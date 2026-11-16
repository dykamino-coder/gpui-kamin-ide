// In-process Grep for small trees: produces the stdout and exit code that the
// Grep tool's ripgrep command (`ripgrepArgs`) would, so the rest of the tool
// treats both the same. Exactness rules:
// - only arguments with a replicated meaning (no type, multiline or context,
//   slashless globs, no ripgrep config file); the regex inside the subset of
//   rg-regex.ts; ignore files through rg-ignore.ts;
// - a file whose bytes ripgrep reads differently (a BOM, a NUL byte, invalid
//   UTF-8) is decided in-process only when the JS regex matches none of its
//   lines: ripgrep's matches on such bytes are a subset of the JS ones, so
//   "no match" is certain, and any match hands the search to ripgrep;
// - each file's block of output is ripgrep's byte for byte; the order of
//   files is the walk's (walk.ts), as ripgrep's own parallel order is arbitrary.
// Returns null whenever ripgrep must run.
import fs from 'node:fs'
import path from 'node:path'
import { isUtf8 } from 'node:buffer'
import { ignoreContext, ignoreVerdict, Overrides, dirRules, type DirRules } from './rg-ignore'
import { lineMatcher } from './rg-regex'
import { defaultBudget, walk, WalkAbort, type WalkBudget } from './walk'

export interface LocalGrepOptions {
  pattern: string
  searchPath: string
  outputMode: 'content' | 'files_with_matches' | 'count'
  /** The `--glob` values after the VCS exclusions, in argument order. */
  globs: string[]
  vcsDirectories: string[]
  fileType?: string
  caseInsensitive: boolean
  showLineNumbers: boolean
  multiline: boolean
  afterContext?: number
  beforeContext?: number
  contextLines?: number
  maxColumns: number
}

export interface LocalGrepOutput {
  stdout: string
  code: 0 | 1
}

function supported(o: LocalGrepOptions): boolean {
  if (o.fileType || o.multiline) return false
  if (o.contextLines !== undefined || o.afterContext !== undefined || o.beforeContext !== undefined) return false
  if (process.env.RIPGREP_CONFIG_PATH) return false
  return o.globs.every((g) => !g.replace(/^!/, '').replace(/\/$/, '').includes('/'))
}

function hasBom(b: Buffer): boolean {
  return (b[0] === 0xef && b[1] === 0xbb && b[2] === 0xbf) || (b[0] === 0xff && b[1] === 0xfe) || (b[0] === 0xfe && b[1] === 0xff)
}

/** Output of one file, '' when it has no match; throws WalkAbort when ripgrep must decide. */
function searchFile(o: LocalGrepOptions, re: RegExp, file: string, withName: boolean, budget: { bytes: number }): string {
  let buf: Buffer
  try {
    buf = fs.readFileSync(file)
  } catch {
    throw new WalkAbort()
  }
  budget.bytes -= buf.length
  if (budget.bytes < 0) throw new WalkAbort()
  const text = buf.toString('utf8')
  const lines = text.split('\n')
  const terminated = lines[lines.length - 1] === ''
  if (terminated) lines.pop()
  const exact = !hasBom(buf) && !buf.includes(0) && isUtf8(buf)
  let out = ''
  let count = 0
  const prefix = withName ? `${file}:` : ''
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!
    if (!re.test(line)) continue
    if (!exact) throw new WalkAbort()
    count++
    if (o.outputMode === 'files_with_matches') return `${file}\n`
    if (o.outputMode === 'content') {
      // The column limit counts the line's bytes with its terminator.
      const shown = Buffer.byteLength(line) + (i < lines.length - 1 || terminated ? 1 : 0) > o.maxColumns ? '[Omitted long matching line]' : line
      out += `${prefix}${o.showLineNumbers ? `${i + 1}:` : ''}${shown}\n`
    }
  }
  if (count === 0) return ''
  return o.outputMode === 'count' ? `${prefix}${count}\n` : out
}

/** What `rg <ripgrepArgs(o)>` prints, or null when ripgrep must run. */
export function localGrep(o: LocalGrepOptions, budget: WalkBudget = defaultBudget()): LocalGrepOutput | null {
  if (!supported(o) || !path.isAbsolute(o.searchPath)) return null
  const re = lineMatcher(o.pattern, o.caseInsensitive)
  if (!re) return null
  const overrides = Overrides.compile([...o.vcsDirectories.map((d) => `!${d}`), ...o.globs])
  if (!overrides) return null
  let stat: fs.Stats
  try {
    stat = fs.lstatSync(o.searchPath)
  } catch {
    return null
  }
  const bytes = { bytes: budget.bytes }
  try {
    if (stat.isFile()) {
      // An explicit file is searched whatever the ignore files say; globs and
      // a VCS path segment are left to ripgrep.
      const segs = o.searchPath.split(/[\\/]/)
      if (o.globs.length || segs.some((s) => o.vcsDirectories.includes(s))) return null
      const stdout = searchFile(o, re, o.searchPath, false, bytes)
      return { stdout, code: stdout ? 0 : 1 }
    }
    if (!stat.isDirectory()) return null
    const ctx = ignoreContext(o.searchPath)
    if (!ctx) return null
    const chain: DirRules[] = [...ctx.parents]
    let stdout = ''
    const done = walk(o.searchPath, budget, {
      enter: (abs, rel, names) => {
        if (rel && names.includes('.git')) throw new WalkAbort()
        const r = dirRules(abs, ctx.useGit, names)
        if (!r) throw new WalkAbort()
        chain.push(r)
      },
      leave: () => void chain.pop(),
      dir: (abs, rel) => {
        const ov = overrides.verdict(rel, true)
        if (ov) return ov === 'whitelist'
        return ignoreVerdict(chain, ctx, abs, true) !== 'ignore'
      },
      file: (abs, rel) => {
        const ov = overrides.verdict(rel, false)
        if (ov === 'ignore' || (ov === null && ignoreVerdict(chain, ctx, abs, false) === 'ignore')) return
        stdout += searchFile(o, re, abs, true, bytes)
        if (performance.now() > budget.deadline) throw new WalkAbort()
      },
    })
    if (!done) return null
    return { stdout, code: stdout ? 0 : 1 }
  } catch (err) {
    if (err instanceof WalkAbort) return null
    throw err
  }
}
