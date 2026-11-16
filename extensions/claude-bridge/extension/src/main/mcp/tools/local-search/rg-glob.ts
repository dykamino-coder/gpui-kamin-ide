// ripgrep's glob meaning (gitignore-style: `--glob` overrides and ignore-file
// lines), compiled to a JS RegExp over a `/`-separated path relative to the
// glob's root. Only a subset whose meaning is certain is compiled; anything
// else returns null and the caller hands the whole search to ripgrep, so the
// in-process search never matches "almost like" ripgrep.

/** A compiled gitignore-style glob. */
export interface RgGlob {
  /** Matches the root-relative path (no leading `/`). */
  regex: RegExp
  /** The line ended with `/`: it matches directories only. */
  dirOnly: boolean
}

const SEGMENT_CHARS = /^[^\\{}[\]]*$/

/** One path segment's glob (no `/`), or null when outside the subset. */
function segment(seg: string): string | null {
  if (seg.includes('**')) return null
  let out = ''
  for (let i = 0; i < seg.length; i++) {
    const c = seg[i]!
    if (c === '*') out += '[^/]*'
    else if (c === '?') out += '[^/]'
    else if (c === '[') {
      const end = seg.indexOf(']', i + 2)
      if (end === -1) return null
      let body = seg.slice(i + 1, end)
      let negated = false
      if (body.startsWith('!') || body.startsWith('^')) {
        negated = true
        body = body.slice(1)
      }
      // `[]…]`, nested classes, POSIX classes and escapes differ between
      // globset and JS; ranges and plain members do not.
      if (!body || /[[\]\\/]/.test(body)) return null
      out += `[${negated ? '^/' : ''}${body.replace(/[\^]/g, '\\^')}]`
      i = end
    } else if (c === '{') {
      const end = seg.indexOf('}', i)
      if (end === -1) return null
      const alts = seg.slice(i + 1, end).split(',')
      if (alts.length < 2) return null
      const parts: string[] = []
      for (const alt of alts) {
        if (!SEGMENT_CHARS.test(alt)) return null
        const p = segment(alt)
        if (p === null) return null
        parts.push(p)
      }
      out += `(?:${parts.join('|')})`
      i = end
    } else if (c === ']' || c === '}' || c === '\\') return null
    else out += c.replace(/[.+^$()|]/g, '\\$&')
  }
  return out
}

/**
 * Compiles one gitignore-style glob. A glob without a `/` (other than a
 * trailing one) matches the name at any depth; with one, it is anchored to
 * the root. `**` is special only as a whole segment.
 */
export function compileRgGlob(raw: string): RgGlob | null {
  let glob = raw
  const dirOnly = glob.endsWith('/')
  if (dirOnly) glob = glob.slice(0, -1)
  if (!glob || glob.includes('//')) return null
  const anchored = glob.includes('/')
  if (glob.startsWith('/')) glob = glob.slice(1)
  if (!glob) return null
  const segs = glob.split('/')
  if (segs.some((s, i) => s === '**' && segs[i + 1] === '**')) return null
  let re = anchored ? '' : '(?:.*/)?'
  for (let i = 0; i < segs.length; i++) {
    const seg = segs[i]!
    const last = i === segs.length - 1
    if (seg === '**') {
      if (segs.length === 1) re += '.*'
      else if (i === 0) re += '(?:.*/)?'
      else if (last) re += '/.*'
      else re += '(?:/.*)?/'
      continue
    }
    const s = segment(seg)
    if (s === null) return null
    const prevStar = i > 0 && segs[i - 1] === '**'
    if (i > 0 && !prevStar) re += '/'
    re += s
  }
  try {
    return { regex: new RegExp(`^${re}$`, 'u'), dirOnly }
  } catch {
    return null
  }
}
