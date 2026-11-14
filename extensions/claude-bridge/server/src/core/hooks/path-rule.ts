// The file rule of Claude Code permission rules (`Edit(src/**)`,
// `Read(//etc/**)`, `Write(~/notes/*.md)`) for a hook's `if` condition. As in
// the CLI the rule is a gitignore pattern relative to its root, and a rule
// without a wildcard or with a leading `*` is also compared with the whole
// path. The paths are those of the client: a Windows path is compared without
// case, with `/` separators and its drive written as `/c/…` for `//` rules.

import path from 'node:path'
import ignore from 'ignore'
import { wildcardMatches } from './rule-matchers'

export interface PathRuleContext {
  /** Working directory of the session on the client (rule root for `/x`, `x`). */
  cwd?: string
  /** Home directory of the client (rule root for `~/x`), when known. */
  home?: string
}

const WINDOWS_PATH = /^[A-Za-z]:[\\/]/

/** Forward slashes, and a drive letter as the first segment (`C:\a` → `/c/a`). */
function normalize(p: string): string {
  const slashed = p.replaceAll('\\', '/')
  return WINDOWS_PATH.test(p) ? `/${slashed[0]!.toLowerCase()}${slashed.slice(2)}` : slashed
}

function relative(root: string, target: string): string | null {
  const base = root.replace(/\/+$/, '')
  if (base === '') return target.startsWith('/') ? target.slice(1) : null
  if (target === base) return null
  return target.startsWith(base + '/') ? target.slice(base.length + 1) : null
}

/** A rule's root and its pattern relative to that root (`//x` absolute,
 *  `~/x` home, `/x` and `./x` the working directory, `x` anywhere below it). */
function splitRoot(rule: string, ctx: PathRuleContext): { root: string | null; pattern: string } {
  if (rule.startsWith('//')) return { root: '/', pattern: rule.slice(1) }
  if (rule.startsWith('~/')) return { root: ctx.home ? normalize(ctx.home) : null, pattern: rule.slice(1) }
  const cwd = ctx.cwd ? normalize(ctx.cwd) : null
  if (rule.startsWith('./')) return { root: cwd, pattern: rule.slice(1) }
  return { root: cwd, pattern: rule }
}

/** `dir/**` names the directory itself, as in the CLI's allow-rule form. */
function directoryForm(pattern: string): string {
  if (!pattern.endsWith('/**')) return pattern
  const dir = pattern.slice(0, -3)
  if (!/[^/]/.test(dir)) return '/**'
  return dir.includes('/') || /^[!#]/.test(dir) ? dir : '/' + dir
}

/** Use the same gitignore engine as the native CLI, including directory and
 * escaped pattern semantics. */
export function gitignoreMatches(pattern: string, relPath: string, ignoreCase: boolean): boolean {
  return ignore.isPathValid(relPath) && ignore({ ignoreCase }).add(pattern).test(relPath).ignored
}

function hasUnescapedStar(rule: string): boolean {
  if (rule.endsWith(':*')) return false
  for (let i = 0; i < rule.length; i++) {
    if (rule[i] !== '*') continue
    let backslashes = 0
    for (let j = i - 1; j >= 0 && rule[j] === '\\'; j--) backslashes++
    if (backslashes % 2 === 0) return true
  }
  return false
}

export function pathRuleMatches(rule: string, filePath: string, ctx: PathRuleContext = {}): boolean {
  const windows = WINDOWS_PATH.test(filePath) || WINDOWS_PATH.test(ctx.cwd ?? '')
  const cwd = ctx.cwd ? normalize(ctx.cwd) : undefined
  const target = path.posix.resolve(cwd ?? '/', normalize(filePath))
  const { root, pattern } = splitRoot(rule, ctx)
  const rel =
    root === null ? null : relative(windows ? root.toLowerCase() : root, windows ? target.toLowerCase() : target)
  if (rel !== null && gitignoreMatches(directoryForm(pattern), rel, windows)) return true
  const trimmed = rule.trim()
  if (trimmed.startsWith('*') || (!hasUnescapedStar(trimmed) && !trimmed.endsWith(':*'))) {
    return wildcardMatches(rule, filePath, windows)
  }
  return false
}
