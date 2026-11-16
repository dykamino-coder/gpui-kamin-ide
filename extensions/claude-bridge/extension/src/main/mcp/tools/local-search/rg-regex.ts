// ripgrep's default regex (the Rust `regex` crate, Unicode mode) translated
// to a JS `u` RegExp that accepts exactly the same lines of valid UTF-8 text.
// Only whether a line matches is observable in Grep's output (whole lines,
// line counts, file names), so greediness and capture groups do not matter.
// The translated subset: literals, `.`, anchors, classes without nesting or
// set operations, groups, alternation, counted and lazy repetition, and the
// Perl classes `\w \d \s \b` with their Unicode meaning. Anything else
// returns null and the search goes to ripgrep.

/** Rust `\w`: Alphabetic, marks, decimal digits, connector punctuation, joiners. */
const WORD = '\\p{Alphabetic}\\p{M}\\p{Nd}\\p{Pc}\\p{Join_Control}'
const PERL: Record<string, string> = {
  w: `[${WORD}]`,
  W: `[^${WORD}]`,
  d: '\\p{Nd}',
  D: '\\P{Nd}',
  s: '\\p{White_Space}',
  S: '\\P{White_Space}',
}
const PERL_IN_CLASS: Record<string, string> = { w: WORD, d: '\\p{Nd}', s: '\\p{White_Space}' }
const W = `[${WORD}]`
const BOUNDARY = `(?:(?<=${W})(?!${W})|(?<!${W})(?=${W}))`
const NOT_BOUNDARY = `(?:(?<=${W})(?=${W})|(?<!${W})(?!${W}))`
/** Characters that are meta outside a class in both dialects. */
const META = '.+*?()|[]{}^$\\'
// `\n` is absent: ripgrep refuses a pattern that names the line terminator.
const ESCAPED_LITERAL: Record<string, string> = { t: '\\t', r: '\\r' }

function quantifier(src: string, i: number): number {
  // Returns the length of a `{n}`, `{n,}` or `{n,m}` at i, or 0.
  const m = /^\{(\d{1,4})(,(\d{1,4})?)?\}/.exec(src.slice(i))
  if (!m) return 0
  if (m[3] !== undefined && Number(m[3]) < Number(m[1])) return 0
  return m[0].length
}

function translateClass(src: string, start: number): { out: string; end: number } | null {
  let i = start + 1
  let out = '['
  if (src[i] === '^') {
    out += '^'
    i++
  }
  if (src[i] === ']') return null
  for (; i < src.length; i++) {
    const c = src[i]!
    if (c === ']') return out.length > 1 && out !== '[^' ? { out: out + ']', end: i } : null
    if (c === '[') return null
    if ((c === '&' || c === '-' || c === '~') && src[i + 1] === c) return null
    if (c === '\\') {
      const e = src[i + 1]
      if (e === undefined) return null
      if (PERL_IN_CLASS[e]) out += PERL_IN_CLASS[e]
      else if (ESCAPED_LITERAL[e]) out += ESCAPED_LITERAL[e]
      else if ('-]\\^[.*+?(){}|$'.includes(e)) out += '\\' + e
      else return null
      i++
      continue
    }
    out += c
  }
  return null
}

/** The JS source of a Rust regex pattern, or null outside the subset. */
export function translateRustRegex(src: string): string | null {
  let out = ''
  let depth = 0
  // Whether the previous token can take a quantifier.
  let atom = false
  for (let i = 0; i < src.length; i++) {
    const c = src[i]!
    if (c === '\\') {
      const e = src[i + 1]
      if (e === undefined) return null
      i++
      if (PERL[e]) out += PERL[e]
      else if (e === 'b' || e === 'B') {
        out += e === 'b' ? BOUNDARY : NOT_BOUNDARY
        atom = false
        continue
      } else if (ESCAPED_LITERAL[e]) out += ESCAPED_LITERAL[e]
      else if (META.includes(e)) out += '\\' + e
      else return null
      atom = true
    } else if (c === '[') {
      const cls = translateClass(src, i)
      if (!cls) return null
      out += cls.out
      i = cls.end
      atom = true
    } else if (c === '(') {
      if (src[i + 1] === '?') {
        if (src[i + 2] !== ':') return null
        out += '(?:'
        i += 2
      } else out += '('
      depth++
      atom = false
    } else if (c === ')') {
      if (depth === 0) return null
      depth--
      out += ')'
      atom = true
    } else if (c === '*' || c === '+' || c === '?') {
      if (!atom) return null
      out += c
      if (src[i + 1] === '?') {
        out += '?'
        i++
      }
      if ('*+?{'.includes(src[i + 1] ?? '')) return null
      atom = false
    } else if (c === '{') {
      const len = quantifier(src, i)
      if (!atom || len === 0) return null
      out += src.slice(i, i + len)
      i += len - 1
      if (src[i + 1] === '?') {
        out += '?'
        i++
      }
      if ('*+?{'.includes(src[i + 1] ?? '')) return null
      atom = false
    } else if (c === '}' || c === ']') return null
    else if (c === '.') {
      // Rust `.` excludes only `\n`; JS `.` also excludes `\r` and U+2028/9.
      out += '[^\\n]'
      atom = true
    } else if (c === '^' || c === '$') {
      // Anchors only where they bound a whole alternative: ripgrep's
      // line-oriented search treats anchors elsewhere (`$^`, `\b^`) its own way.
      const before = src.slice(0, i)
      const after = src[i + 1]
      if (c === '^' && !(i === 0 || before.endsWith('|') || before.endsWith('(') || before.endsWith('(?:'))) return null
      if (c === '$' && !(after === undefined || after === '|' || after === ')')) return null
      if (before.endsWith('\\b') || before.endsWith('\\B') || src.startsWith('\\b', i + 1) || src.startsWith('\\B', i + 1)) return null
      out += c
      atom = false
    } else if (c === '|') {
      out += c
      atom = false
    } else {
      out += c
      atom = true
    }
  }
  if (depth !== 0) return null
  return out
}

/** A RegExp testing one line (without its `\n`), or null outside the subset. */
export function lineMatcher(pattern: string, caseInsensitive: boolean): RegExp | null {
  const src = translateRustRegex(pattern)
  if (src === null) return null
  try {
    return new RegExp(src, caseInsensitive ? 'iu' : 'u')
  } catch {
    return null
  }
}
