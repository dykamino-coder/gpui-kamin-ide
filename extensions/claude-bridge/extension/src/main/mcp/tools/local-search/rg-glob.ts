// ripgrep's glob meaning (gitignore-style: `--glob` overrides and ignore-file
// lines), compiled to a JS RegExp over a `/`-separated path relative to the
// glob's root. A port of the `ignore` crate's `GitignoreBuilder::add_line` and
// of `globset`'s parser and regex generation (literal separator, backslash
// escapes). Whatever differs between the ripgrep versions in use (nested or
// empty alternates, `**` inside alternates, an unclosed class, a `\/` before
// the trailing slash) or is a glob error returns null, and the caller hands
// the whole search to ripgrep, so the in-process search never matches
// "almost like" ripgrep.

/** A compiled gitignore-style glob. */
export interface RgGlob {
  /** Matches the root-relative path (no leading `/`). */
  regex: RegExp
  /** The line ended with `/`: it matches directories only. */
  dirOnly: boolean
  /** globset matches bytes: `?` and classes see one byte of a non-ASCII
   *  character, so the regex is exact only on ASCII paths. */
  asciiOnly: boolean
}

/** One gitignore-style line: the glob and whether it began with `!`. */
export interface RgGlobLine {
  glob: RgGlob
  /** `!glob`: re-includes in an ignore file, excludes as an override. */
  bang: boolean
}

type Token =
  | { k: 'lit'; c: string }
  | { k: 'any' }
  | { k: 'star' }
  | { k: 'rprefix' }
  | { k: 'rsuffix' }
  | { k: 'rzom' }
  | { k: 'class'; negated: boolean; ranges: [string, string][] }
  | { k: 'alt'; branches: Token[][] }

class Refused extends Error {}

/** globset's tokenizer with `backslash_escape(true)`; throws Refused. */
function tokenize(glob: string): Token[] {
  const chars = Array.from(glob)
  let pos = 0
  let cur: string | undefined
  let prev: string | undefined
  const bump = (): string | undefined => {
    prev = cur
    cur = chars[pos++]
    return cur
  }
  const peek = (): string | undefined => chars[pos]
  const branches: Token[][] = [[]]
  const altStack: number[] = []
  const push = (t: Token): void => void branches[branches.length - 1]!.push(t)
  const inAlt = (): boolean => altStack.length > 0

  const star = (): void => {
    const before = prev
    if (peek() !== '*') return push({ k: 'star' })
    bump()
    if (inAlt()) throw new Refused()
    const last = branches[branches.length - 1]!
    if (last.length === 0) {
      if (peek() !== undefined && peek() !== '/') {
        push({ k: 'star' })
        push({ k: 'star' })
      } else {
        push({ k: 'rprefix' })
        bump()
      }
      return
    }
    if (before !== '/') {
      push({ k: 'star' })
      push({ k: 'star' })
      return
    }
    let suffix: boolean
    if (peek() === undefined) {
      bump()
      suffix = true
    } else if (peek() === '/') {
      bump()
      suffix = false
    } else {
      push({ k: 'star' })
      push({ k: 'star' })
      return
    }
    const popped = last.pop()!
    if (popped.k === 'rprefix' || popped.k === 'rsuffix') push(popped)
    else push({ k: suffix ? 'rsuffix' : 'rzom' })
  }

  const cls = (): void => {
    let negated = false
    if (peek() === '!' || peek() === '^') {
      bump()
      negated = true
    }
    const ranges: [string, string][] = []
    let first = true
    let inRange = false
    const extend = (c: string): void => {
      const r = ranges[ranges.length - 1]!
      r[1] = c
      if (r[1].codePointAt(0)! < r[0].codePointAt(0)!) throw new Refused()
    }
    for (;;) {
      const c = bump()
      // Unclosed: a literal `[` in newer globset, an error in older ones.
      if (c === undefined) throw new Refused()
      if (c === ']' && !first) break
      if (c === ']') ranges.push([']', ']'])
      else if (c === '-') {
        if (first) ranges.push(['-', '-'])
        else if (inRange) {
          extend('-')
          inRange = false
        } else inRange = true
      } else {
        if (inRange) extend(c)
        else ranges.push([c, c])
        inRange = false
      }
      first = false
    }
    if (inRange) ranges.push(['-', '-'])
    // A non-ASCII member is a byte sequence to globset, not one character.
    if (ranges.some(([a, b]) => a > '\x7f' || b > '\x7f')) throw new Refused()
    push({ k: 'class', negated, ranges })
  }

  for (let c = bump(); c !== undefined; c = bump()) {
    if (c === '?') push({ k: 'any' })
    else if (c === '*') star()
    else if (c === '[') cls()
    else if (c === '{') {
      if (inAlt()) throw new Refused()
      altStack.push(branches.length)
      branches.push([])
    } else if (c === '}') {
      const start = altStack.pop()
      if (start === undefined) throw new Refused()
      const alts = branches.splice(start)
      if (alts.some((b) => b.length === 0)) throw new Refused()
      push({ k: 'alt', branches: alts })
    } else if (c === ',') {
      if (inAlt()) branches.push([])
      else push({ k: 'lit', c })
    } else if (c === '\\') {
      const n = bump()
      if (n === undefined) throw new Refused()
      push({ k: 'lit', c: n })
    } else push({ k: 'lit', c })
  }
  if (inAlt()) throw new Refused()
  return branches[0]!
}

const escLit = (c: string): string => c.replace(/[\\^$.*+?()[\]{}|/]/g, '\\$&')
const escClass = (c: string): string => c.replace(/[\\\]\[^-]/g, '\\$&')
// globset's regex lets `.` match a newline.
const ANY = '[\\s\\S]'

function toRegex(tokens: Token[]): string {
  let re = ''
  for (const t of tokens) {
    if (t.k === 'lit') re += escLit(t.c)
    else if (t.k === 'any') re += '[^/]'
    else if (t.k === 'star') re += '[^/]*'
    else if (t.k === 'rprefix') re += `(?:/?|${ANY}*/)`
    else if (t.k === 'rsuffix') re += `/${ANY}*`
    else if (t.k === 'rzom') re += `(?:/|/${ANY}*/)`
    else if (t.k === 'class') {
      re += `[${t.negated ? '^' : ''}${t.ranges.map(([a, b]) => (a === b ? escClass(a) : `${escClass(a)}-${escClass(b)}`)).join('')}]`
    } else re += `(?:${t.branches.map(toRegex).join('|')})`
  }
  return re
}

const byteSensitive = (tokens: Token[]): boolean =>
  tokens.some((t) => t.k === 'any' || t.k === 'class' || (t.k === 'alt' && t.branches.some(byteSensitive)))

/** globset's `Glob` for an already rewritten gitignore glob, or null. */
function buildGlob(actual: string, dirOnly: boolean): RgGlob | null {
  let tokens: Token[]
  try {
    tokens = tokenize(actual)
  } catch (err) {
    if (err instanceof Refused) return null
    throw err
  }
  const body = tokens.length === 1 && tokens[0]!.k === 'rprefix' ? `${ANY}*` : toRegex(tokens)
  try {
    return { regex: new RegExp(`^${body}$`, 'u'), dirOnly, asciiOnly: byteSensitive(tokens) }
  } catch {
    return null
  }
}

const TRAILING_WS = /\p{White_Space}+$/u

/**
 * One gitignore-style line as `GitignoreBuilder::add_line` reads it: 'skip'
 * for a comment or a blank line, null when ripgrep must decide.
 */
export function parseRgGlobLine(raw: string): RgGlobLine | 'skip' | null {
  let line = raw
  if (line.startsWith('#')) return 'skip'
  if (!line.endsWith('\\ ')) line = line.replace(TRAILING_WS, '')
  if (!line) return 'skip'
  let bang = false
  let absolute = false
  if (line.startsWith('\\!') || line.startsWith('\\#')) {
    line = line.slice(1)
    absolute = line.startsWith('/')
  } else {
    if (line.startsWith('!')) {
      bang = true
      line = line.slice(1)
    }
    if (line.startsWith('/')) {
      line = line.slice(1)
      absolute = true
    }
  }
  let dirOnly = false
  if (line.endsWith('/')) {
    dirOnly = true
    line = line.slice(0, -1)
    // Older ripgrep keeps the escape of an escaped trailing slash.
    if (line.endsWith('\\')) return null
  }
  let actual = line
  if (!absolute && !line.includes('/') && !(actual.startsWith('**/') || actual === '**')) actual = `**/${actual}`
  if (actual.endsWith('/**')) actual += '/*'
  const glob = buildGlob(actual, dirOnly)
  return glob ? { glob, bang } : null
}

/** A single glob as an ignore-file line would read it; null for anything
 *  else (including a `!` line, a comment or a blank). */
export function compileRgGlob(raw: string): RgGlob | null {
  const r = parseRgGlobLine(raw)
  return r && r !== 'skip' && !r.bang ? r.glob : null
}

/** Whether `rel` matches `g`; undefined when only ripgrep can tell. */
export function rgGlobMatch(g: RgGlob, rel: string): boolean | undefined {
  if (g.asciiOnly && /[^\x00-\x7f]/.test(rel)) return undefined
  return g.regex.test(rel)
}
