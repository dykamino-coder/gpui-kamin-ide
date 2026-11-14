// File listing of the Glob tool. ripgrep walks the tree on all cores and
// matches the pattern in Rust, as native Glob does, and the newest paths are
// picked from our own stat pass: native asks ripgrep for `--sort=modified`,
// which makes the walk single-threaded and was 2x (Linux) to 8x (Windows)
// slower on the same trees. Without ripgrep, fast-glob walks in-process with
// the same pattern meaning. fast-glob is imported statically so the bundler
// inlines it: packaged builds carry no node_modules.
import { spawn } from 'node:child_process'
import fs from 'node:fs'
import fsp from 'node:fs/promises'
import path from 'node:path'
import fg from 'fast-glob'
import { STAT_CONCURRENCY } from './fs-shared'
import { ripgrepPath } from './ripgrep-path'

/** Paths listed before the newest are chosen; a broader pattern is reported
 *  as capped instead of being stat'ed without bound. */
export const MAX_GLOB_LISTED = 50_000

/** Never listed unless the pattern or path names them: VCS internals and
 *  installed dependencies, which bury a project's own files. */
const SKIPPED_DIRECTORIES = ['.git', '.svn', '.hg', '.bzr', '.jj', '.sl', 'node_modules']

export interface GlobQuery {
  /** Directory the pattern is matched under. */
  base: string
  /** Pattern relative to `base`, in ripgrep (gitignore) syntax. */
  pattern: string
  /** Directory names excluded from the walk. */
  skipped: string[]
}

const GLOB_CHARS = /[*?[\]{}!]/

/** Normalises a model pattern. A literal leading part of an absolute or
 *  `../` pattern becomes the base, and the rest is anchored to it, so
 *  `/repo/src/*.ts` means what it says rather than matching nothing. A
 *  pattern without `/` matches at any depth (ripgrep and native Glob meaning). */
export function globQuery(rawPattern: string, base: string, platform: NodeJS.Platform = process.platform): GlobQuery {
  let pattern = platform === 'win32' ? rawPattern.replace(/\\/g, '/') : rawPattern
  while (pattern.startsWith('./')) pattern = pattern.slice(2)
  const absolute = pattern.startsWith('/') || /^[a-zA-Z]:\//.test(pattern)
  if (absolute || pattern.startsWith('../')) {
    const segments = pattern.split('/')
    let i = 0
    while (i < segments.length - 1 && !GLOB_CHARS.test(segments[i]!)) i++
    let prefix = segments.slice(0, i).join('/')
    if (prefix === '') prefix = '/'
    else if (/^[a-zA-Z]:$/.test(prefix)) prefix += '/'
    base = path.resolve(base, prefix)
    pattern = '/' + segments.slice(i).join('/')
  }
  const named = `${base}/${pattern}`.replace(/\\/g, '/')
  const skipped = SKIPPED_DIRECTORIES.filter((d) => !named.split('/').includes(d))
  return { base, pattern, skipped }
}

export interface Listing {
  files: string[]
  capped: boolean
}

/** Arguments of the ripgrep listing (exported for tests). */
export function ripgrepListArgs(q: GlobQuery): string[] {
  const args = ['--files', '--null', '--hidden', '--no-ignore', '--no-config', '--follow', '--glob', q.pattern]
  for (const d of q.skipped) args.push('--glob', `!${d}`)
  args.push('--', q.base)
  return args
}

function listWithRipgrep(rg: string, q: GlobQuery, limit: number, signal?: AbortSignal): Promise<Listing> {
  return new Promise<Listing>((resolve, reject) => {
    // ripgrep matches a glob with a `/` against the path relative to its
    // working directory, so the walk starts from the base itself.
    const child = spawn(rg, ripgrepListArgs(q), { cwd: q.base, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true, signal })
    const files: string[] = []
    let capped = false
    let remainder: Buffer = Buffer.alloc(0)
    let stderr = ''
    const toPath = process.platform === 'win32' ? (p: string) => p.replace(/\\/g, '/') : (p: string) => p
    child.stdout.on('data', (chunk: Buffer) => {
      if (capped) return
      const data = remainder.length ? Buffer.concat([remainder, chunk]) : chunk
      let start = 0
      for (let end = data.indexOf(0, start); end !== -1; end = data.indexOf(0, start)) {
        files.push(toPath(data.subarray(start, end).toString('utf8')))
        start = end + 1
        if (files.length >= limit) {
          capped = true
          child.kill()
          break
        }
      }
      remainder = capped ? Buffer.alloc(0) : data.subarray(start)
    })
    child.stderr.on('data', (chunk: Buffer) => {
      if (stderr.length < 4096) stderr += chunk.toString('utf8')
    })
    child.on('error', reject)
    child.on('close', (code) => {
      // 1: nothing matched. 2: some entries were unreadable; the rest is valid.
      if (capped || code === 0 || code === 1 || (code === 2 && files.length > 0)) resolve({ files, capped })
      else reject(new Error(stderr.trim() || `rg --files exited with code ${code}`))
    })
  })
}

async function listWithFastGlob(q: GlobQuery, limit: number): Promise<Listing> {
  const anchored = q.pattern.startsWith('/')
  const pattern = anchored ? q.pattern.slice(1) : q.pattern.includes('/') ? q.pattern : `**/${q.pattern}`
  const files: string[] = []
  let capped = false
  const stream = fg.stream(pattern, {
    cwd: q.base,
    absolute: true,
    dot: true,
    onlyFiles: true,
    followSymbolicLinks: true,
    ignore: q.skipped.map((d) => `**/${d}/**`),
  })
  for await (const entry of stream) {
    files.push(entry as string)
    if (files.length >= limit) {
      capped = true
      break
    }
  }
  if (capped) (stream as unknown as { destroy?: () => void }).destroy?.()
  return { files, capped }
}

/** Lists the files matching `q`: ripgrep when available, fast-glob otherwise. */
export async function listGlob(q: GlobQuery, limit = MAX_GLOB_LISTED, signal?: AbortSignal): Promise<Listing> {
  const rg = await ripgrepPath()
  return rg ? listWithRipgrep(rg, q, limit, signal) : listWithFastGlob(q, limit)
}

/** Sorts newest first (ties by path). Synchronous stat is several times
 *  faster than the thread pool on Linux/macOS page-cached metadata; on
 *  Windows, where each stat opens the file, parallel requests win. */
export async function newestFirst(files: string[]): Promise<string[]> {
  const mtimes = new Float64Array(files.length)
  if (process.platform === 'win32') {
    for (let i = 0; i < files.length; i += STAT_CONCURRENCY * 2) {
      const batch = files.slice(i, i + STAT_CONCURRENCY * 2)
      const stats = await Promise.all(batch.map((f) => fsp.stat(f).then((s) => s.mtimeMs, () => 0)))
      mtimes.set(stats, i)
    }
  } else {
    for (let i = 0; i < files.length; i++) {
      try {
        mtimes[i] = fs.statSync(files[i]!, { throwIfNoEntry: false })?.mtimeMs ?? 0
      } catch {
        mtimes[i] = 0
      }
      // Yield now and then so a huge listing does not hold the host's loop.
      if (i % 4096 === 4095) await new Promise((r) => setImmediate(r))
    }
  }
  const order = files.map((_, i) => i)
  order.sort((a, b) => mtimes[b]! - mtimes[a]! || (files[a]! < files[b]! ? -1 : files[a]! > files[b]! ? 1 : 0))
  return order.map((i) => files[i]!)
}
