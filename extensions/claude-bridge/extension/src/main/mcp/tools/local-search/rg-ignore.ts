// ripgrep's path filters for an in-process walk: `--glob` overrides and the
// ignore files (.rgignore, .ignore, .gitignore, .git/info/exclude) with the
// precedence of ripgrep's `ignore` crate: overrides first; then by file kind,
// and within a kind the deepest directory with a decisive match; within one
// file the last matching line. A configuration whose meaning is not certain
// (an ignore file above the repository, a nested repository, a configured
// core.excludesFile, a line ripgrep versions read differently) returns null: the caller hands the
// search to ripgrep.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { isUtf8 } from 'node:buffer'
import { parseRgGlobLine, rgGlobMatch, type RgGlob } from './rg-glob'
import { WalkAbort } from './walk'

export type Verdict = 'ignore' | 'whitelist' | null

interface Rule {
  glob: RgGlob
  negate: boolean
}

/** Last matching rule of one list; `negate` rules re-include. Throws
 *  WalkAbort when a rule's match on this path is ripgrep's to decide. */
function matchRules(rules: Rule[], rel: string, isDir: boolean): Verdict {
  for (let i = rules.length - 1; i >= 0; i--) {
    const r = rules[i]!
    if (r.glob.dirOnly && !isDir) continue
    const m = rgGlobMatch(r.glob, rel)
    if (m === undefined) throw new WalkAbort()
    if (m) return r.negate ? 'whitelist' : 'ignore'
  }
  return null
}

/** `--glob` overrides: a glob includes, `!glob` excludes; with any including
 *  glob, a file matching none is excluded (directories are still walked). */
export class Overrides {
  private constructor(
    private rules: Rule[],
    private whitelists: number,
  ) {}

  static compile(globs: string[]): Overrides | null {
    const rules: Rule[] = []
    let whitelists = 0
    for (const g of globs) {
      const line = parseRgGlobLine(g)
      if (!line || line === 'skip' || line.glob.dirOnly) return null
      const negate = !line.bang
      if (negate) whitelists++
      rules.push({ glob: line.glob, negate })
    }
    return new Overrides(rules, whitelists)
  }

  verdict(rel: string, isDir: boolean): Verdict {
    const v = matchRules(this.rules, rel, isDir)
    if (v === null && this.whitelists > 0 && !isDir) return 'ignore'
    return v
  }
}

/** Lines of one ignore file as the `ignore` crate reads them (a line ends at
 *  `\n` or `\r\n`), or null when a line is ripgrep's to decide. */
export function parseIgnoreFile(text: string): Rule[] | null {
  // Older ripgrep does not strip a byte order mark.
  if (text.startsWith('\uFEFF')) return null
  const rules: Rule[] = []
  const lines = text.split('\n')
  for (let i = 0; i < lines.length; i++) {
    let line = lines[i]!
    if (i < lines.length - 1 && line.endsWith('\r')) line = line.slice(0, -1)
    const r = parseRgGlobLine(line)
    if (r === null) return null
    if (r !== 'skip') rules.push({ glob: r.glob, negate: r.bang })
  }
  return rules
}

const KINDS = ['.rgignore', '.ignore', '.gitignore'] as const

/** Ignore rules of one directory, by kind (index into KINDS, then exclude). */
export interface DirRules {
  dir: string
  kinds: (Rule[] | null)[]
}

function readRules(file: string): Rule[] | null | undefined {
  let buf: Buffer
  try {
    buf = fs.readFileSync(file)
  } catch (err) {
    return (err as NodeJS.ErrnoException).code === 'ENOENT' ? null : undefined
  }
  if (!isUtf8(buf)) return undefined
  return parseIgnoreFile(buf.toString('utf8')) ?? undefined
}

/** The rules of `dir`; undefined when one of its files cannot be used.
 *  `names` (the directory's listing) spares opening absent files, which is
 *  the dominant cost on Windows. */
export function dirRules(dir: string, useGit: boolean, names?: readonly string[]): DirRules | undefined {
  const kinds: (Rule[] | null)[] = []
  for (const name of KINDS) {
    if ((name === '.gitignore' && !useGit) || (names && !names.includes(name))) {
      kinds.push(null)
      continue
    }
    const r = readRules(path.join(dir, name))
    if (r === undefined) return undefined
    kinds.push(r)
  }
  return { dir, kinds }
}

function exists(p: string): boolean {
  return fs.existsSync(p)
}

/** The global git excludes file (`$XDG_CONFIG_HOME/git/ignore`): its rules,
 *  [] when absent, or undefined when ripgrep must decide (a configured
 *  `core.excludesFile`). ripgrep roots it at its working directory. */
function globalExcludes(): Rule[] | undefined {
  const home = os.homedir()
  const xdg = process.env.XDG_CONFIG_HOME || path.join(home, '.config')
  for (const cfg of [path.join(home, '.gitconfig'), path.join(xdg, 'git', 'config')]) {
    try {
      if (/excludesfile/i.test(fs.readFileSync(cfg, 'utf8'))) return undefined
    } catch {
      // absent
    }
  }
  const rules = readRules(path.join(xdg, 'git', 'ignore'))
  if (rules === undefined) return undefined
  return rules ?? []
}

/** The filters of a search rooted at `root` before its own walk starts. */
export interface IgnoreContext {
  /** True inside a git repository: .gitignore and exclude rules apply. */
  useGit: boolean
  /** Rules of the directories from the repository root down to `root`'s parent. */
  parents: DirRules[]
  /** `.git/info/exclude` of the repository, rooted at the repository. */
  exclude: { dir: string; rules: Rule[] } | null
  /** Unanchored rules of the global git excludes file. */
  global: Rule[]
}

/** Reads the ignore context of `root`, or null when ripgrep must decide. */
export function ignoreContext(root: string): IgnoreContext | null {
  const chain: string[] = []
  for (let d = root; ; ) {
    chain.push(d)
    const up = path.dirname(d)
    if (up === d) break
    d = up
  }
  const repoIdx = chain.findIndex((d) => exists(path.join(d, '.git')))
  if (repoIdx !== -1) {
    try {
      if (!fs.statSync(path.join(chain[repoIdx]!, '.git')).isDirectory()) return null
    } catch {
      return null
    }
  }
  const useGit = repoIdx !== -1
  // Above the repository (or everywhere outside one) any ignore file is left to ripgrep.
  for (const d of chain.slice(useGit ? repoIdx + 1 : 1)) {
    if (KINDS.some((k) => exists(path.join(d, k)))) return null
  }
  const global = useGit ? globalExcludes() : []
  if (global === undefined) return null
  const parents: DirRules[] = []
  for (const d of chain.slice(1, useGit ? repoIdx + 1 : 1).reverse()) {
    const r = dirRules(d, useGit)
    if (!r) return null
    parents.push(r)
  }
  let exclude: IgnoreContext['exclude'] = null
  if (useGit) {
    const repo = chain[repoIdx]!
    const rules = readRules(path.join(repo, '.git', 'info', 'exclude'))
    if (rules === undefined) return null
    if (rules && rules.length) exclude = { dir: repo, rules }
  }
  return { useGit, parents, exclude, global }
}

/** A path as ripgrep's matchers rooted at its working directory (`--glob`
 *  overrides, the global excludes) see it: relative to the working directory
 *  when under it, otherwise whole, so an anchored glob matches nothing there.
 *  Throws WalkAbort when the prefix differs only in case (Windows), where
 *  whether ripgrep strips it is not replicated. */
export function cwdRel(abs: string): string {
  const cwd = process.cwd()
  const prefix = cwd.endsWith(path.sep) ? cwd : cwd + path.sep
  const under = abs.startsWith(prefix)
  if (!under && abs.toLowerCase().startsWith(prefix.toLowerCase())) throw new WalkAbort()
  return (under ? abs.slice(prefix.length) : abs).split(path.sep).join('/')
}

function relTo(dir: string, abs: string): string {
  return path.relative(dir, abs).split(path.sep).join('/')
}

/** The ignore-file verdict of `abs` given the rules of its ancestors (outermost first). */
export function ignoreVerdict(chain: DirRules[], ctx: Pick<IgnoreContext, 'exclude' | 'global'>, abs: string, isDir: boolean): Verdict {
  for (let k = 0; k < KINDS.length; k++) {
    for (let i = chain.length - 1; i >= 0; i--) {
      const rules = chain[i]!.kinds[k]
      if (!rules || !rules.length) continue
      const v = matchRules(rules, relTo(chain[i]!.dir, abs), isDir)
      if (v) return v
    }
  }
  const exclude = ctx.exclude ? matchRules(ctx.exclude.rules, relTo(ctx.exclude.dir, abs), isDir) : null
  return exclude ?? (ctx.global.length ? matchRules(ctx.global, cwdRel(abs), isDir) : null)
}
