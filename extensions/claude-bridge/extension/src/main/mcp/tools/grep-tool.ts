// Grep MCP tool — wraps ripgrep with the arguments of Claude Code's native Grep
// (hidden files searched, VCS directories excluded, long lines capped), a
// head-limit streaming fast path, an EAGAIN single-thread retry, and an
// in-process fallback when no ripgrep binary exists. The fallback either keeps
// ripgrep's meaning for an argument or refuses it explicitly; it never ignores
// an argument silently.

import fsp from 'node:fs/promises'
import path from 'node:path'
import { spawn } from 'node:child_process'
import fg from 'fast-glob'
import { resolveToolPath, resolveWorkingDirectory } from '../../utils/working-directory'
import type { ToolCallContext } from '../executor'
import { type McpResult, textResult, errorResult, MAX_GREP_BYTES, MAX_READ_BYTES, STAT_CONCURRENCY } from './fs-shared'
import { ripgrepPath } from './ripgrep-path'

/** Directories of version control systems never searched (native Grep parity). */
const VCS_DIRECTORIES = ['.git', '.svn', '.hg', '.bzr', '.jj', '.sl']
/** Lines longer than this are replaced by ripgrep's omission notice. */
const MAX_COLUMNS = 500

async function rgBinary(): Promise<string> {
  const rg = await ripgrepPath()
  if (!rg) throw Object.assign(new Error('ripgrep not found'), { code: 'ENOENT' })
  return rg
}

/** Spawn `rg --files` and count matching files (used by Glob's `-l` mode).
 *  Counts lines via byte scan rather than chunk.toString().split — saves on
 *  hot paths where multi-GB repos produce millions of paths. */
export async function ripgrepFileCount(args: string[], target: string, signal: AbortSignal): Promise<number> {
  const rg = await rgBinary()
  return new Promise<number>((resolve, reject) => {
    if (signal.aborted) return reject(new Error('aborted'))
    const child = spawn(rg, [...args, target], {
      stdio: ['ignore', 'pipe', 'ignore'],
      windowsHide: true,
      signal,
    })

    let lines = 0
    child.stdout.on('data', (chunk: Buffer) => {
      for (let i = 0; i < chunk.length; i++) if (chunk[i] === 0x0a) lines++
    })

    let settled = false
    child.on('close', (code) => {
      if (settled) return
      settled = true
      if (code === 0 || code === 1) resolve(lines)
      else reject(new Error(`rg --files exited ${code}`))
    })
    child.on('error', (err) => {
      if (settled) return
      settled = true
      reject(err)
    })
  })
}

/**
 * Stream `rg` matches as soon as each stdout chunk arrives. `onLines` fires
 * per chunk with the lines that are complete in that chunk; partial trailing
 * fragments are carried across chunk boundaries as bytes, so a multi-byte
 * character split by the pipe is decoded whole. Caller can stop early by
 * aborting the signal.
 */
export async function ripgrepStream(
  args: string[],
  target: string,
  signal: AbortSignal,
  onLines: (lines: string[]) => void,
): Promise<void> {
  const rg = await rgBinary()
  return new Promise<void>((resolve, reject) => {
    if (signal.aborted) return reject(new Error('aborted'))
    const child = spawn(rg, [...args, target], {
      stdio: ['ignore', 'pipe', 'ignore'],
      windowsHide: true,
      signal,
    })

    const stripCR = (l: string): string => (l.endsWith('\r') ? l.slice(0, -1) : l)
    let remainder: Buffer = Buffer.alloc(0)
    child.stdout.on('data', (chunk: Buffer) => {
      const data = remainder.length ? Buffer.concat([remainder, chunk]) : chunk
      const end = data.lastIndexOf(0x0a)
      if (end === -1) {
        remainder = data
        return
      }
      remainder = data.subarray(end + 1)
      onLines(data.subarray(0, end).toString('utf8').split('\n').map(stripCR))
    })

    let settled = false
    child.on('close', (code) => {
      if (settled) return
      // Abort raced close — don't flush a torn tail from a killed process.
      // Abort path settles via the 'error' callback below (AbortError).
      if (signal.aborted) return
      settled = true
      if (code === 0 || code === 1) {
        if (remainder.length) onLines([stripCR(remainder.toString('utf8'))])
        resolve()
      } else {
        reject(new Error(`rg exited with code ${code}`))
      }
    })
    child.on('error', (err) => {
      if (settled) return
      settled = true
      reject(err)
    })
  })
}

async function spawnRipgrep(
  args: string[],
  timeoutMs: number,
  singleThread = false,
): Promise<{ stdout: string; stderr: string; truncated: boolean; timedOut: boolean; code: number | null }> {
  const rg = await rgBinary()
  return new Promise((resolve, reject) => {
    const extraArgs = singleThread ? ['-j', '1'] : []
    const child = spawn(rg, [...extraArgs, ...args], {
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    })

    const stdoutChunks: Buffer[] = []
    const stderrChunks: Buffer[] = []
    let totalBytes = 0
    let truncated = false
    let timedOut = false
    let killTimer: NodeJS.Timeout | undefined

    const timer = setTimeout(() => {
      timedOut = true
      if (process.platform === 'win32') {
        child.kill()
      } else {
        child.kill('SIGTERM')
        killTimer = setTimeout(() => {
          if (!child.killed) child.kill('SIGKILL')
        }, 5000)
      }
    }, timeoutMs)

    child.stdout.on('data', (chunk: Buffer) => {
      if (truncated) return
      totalBytes += chunk.length
      if (totalBytes > MAX_GREP_BYTES) {
        truncated = true
        const remaining = MAX_GREP_BYTES - (totalBytes - chunk.length)
        if (remaining > 0) stdoutChunks.push(chunk.subarray(0, remaining))
        if (process.platform === 'win32') child.kill()
        else child.kill('SIGTERM')
      } else {
        stdoutChunks.push(chunk)
      }
    })

    child.stderr.on('data', (chunk: Buffer) => {
      if (stderrChunks.length < 64) stderrChunks.push(chunk)
    })

    let settled = false
    child.on('error', (err) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      if (killTimer) clearTimeout(killTimer)
      reject(err)
    })

    child.on('close', (code) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      if (killTimer) clearTimeout(killTimer)
      resolve({
        stdout: Buffer.concat(stdoutChunks).toString('utf8'),
        stderr: Buffer.concat(stderrChunks).toString('utf8'),
        truncated,
        timedOut,
        code,
      })
    })
  })
}

function isEagainError(stderr: string): boolean {
  return stderr.includes('os error 11') || stderr.includes('Resource temporarily unavailable')
}

/** `glob` may hold several patterns separated by whitespace or commas, except
 *  commas inside a brace alternation (`*.{ts,tsx}`) — native Grep parity. */
export function splitGlobs(glob: string): string[] {
  const out: string[] = []
  for (const part of glob.split(/\s+/).filter(Boolean)) {
    if (part.includes('{') && part.includes('}')) out.push(part)
    else out.push(...part.split(',').filter(Boolean))
  }
  return out
}

export interface GrepOptions {
  pattern: string
  searchPath: string
  outputMode: 'content' | 'files_with_matches' | 'count'
  globs: string[]
  fileType?: string
  caseInsensitive: boolean
  showLineNumbers: boolean
  multiline: boolean
  headLimit: number
  offset: number
  afterContext?: number
  beforeContext?: number
  contextLines?: number
}

function num(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? Math.max(0, Math.floor(value)) : undefined
}

/** Raw Grep tool arguments (not a tool handler itself). */
type GrepInput = Record<string, unknown>

export function grepOptions(input: GrepInput, cwd?: string): GrepOptions | string {
  const pattern = input.pattern
  if (typeof pattern !== 'string' || !pattern) return 'pattern is required'
  const mode = (input.output_mode as string) || 'files_with_matches'
  if (mode !== 'content' && mode !== 'files_with_matches' && mode !== 'count') {
    return `output_mode must be one of content, files_with_matches, count (got "${mode}")`
  }
  return {
    pattern,
    searchPath: input.path ? resolveToolPath(input.path as string, cwd) : resolveWorkingDirectory(cwd),
    outputMode: mode,
    globs: typeof input.glob === 'string' ? splitGlobs(input.glob) : [],
    fileType: typeof input.type === 'string' && input.type ? input.type : undefined,
    caseInsensitive: input['-i'] === true,
    showLineNumbers: input['-n'] !== false,
    multiline: input.multiline === true,
    headLimit: num(input.head_limit) ?? 0,
    offset: num(input.offset) ?? 0,
    afterContext: num(input['-A']),
    beforeContext: num(input['-B']),
    contextLines: num(input['-C']) ?? num(input.context),
  }
}

/** ripgrep arguments of one Grep call; the search path is the last one. */
export function ripgrepArgs(o: GrepOptions): string[] {
  const args = ['--hidden', '--max-columns', String(MAX_COLUMNS)]
  for (const dir of VCS_DIRECTORIES) args.push('--glob', `!${dir}`)
  if (o.outputMode === 'files_with_matches') args.push('--files-with-matches')
  else if (o.outputMode === 'count') args.push('--count')
  if (o.caseInsensitive) args.push('-i')
  if (o.outputMode === 'content' && o.showLineNumbers) args.push('-n')
  if (o.multiline) args.push('-U', '--multiline-dotall')
  if (o.outputMode === 'content') {
    if (o.contextLines !== undefined) args.push('-C', String(o.contextLines))
    else {
      if (o.afterContext !== undefined) args.push('-A', String(o.afterContext))
      if (o.beforeContext !== undefined) args.push('-B', String(o.beforeContext))
    }
  }
  for (const glob of o.globs) args.push('--glob', glob)
  if (o.fileType) args.push('--type', o.fileType)
  // `-e` keeps a pattern that starts with `-` a pattern.
  args.push('-e', o.pattern, '--', o.searchPath)
  return args
}

/** Newest first, like native Grep's files_with_matches. Unstattable paths last. */
async function sortByMtime(files: string[]): Promise<string[]> {
  const times = new Map<string, number>()
  for (let i = 0; i < files.length; i += STAT_CONCURRENCY) {
    await Promise.all(
      files.slice(i, i + STAT_CONCURRENCY).map(async (f) => {
        try {
          times.set(f, (await fsp.stat(f)).mtimeMs)
        } catch {
          times.set(f, 0)
        }
      }),
    )
  }
  return [...files].sort((a, b) => times.get(b)! - times.get(a)! || a.localeCompare(b))
}

function paginate(lines: string[], offset: number, headLimit: number): string[] {
  const from = lines.slice(offset)
  return headLimit > 0 ? from.slice(0, headLimit) : from
}

// 9 min for rg — leaves 1-min headroom under the bridge's 10-min MCP cap.
const RIPGREP_TIMEOUT_MS = 540_000

export async function grepSearch(input: Record<string, unknown>, context?: ToolCallContext): Promise<McpResult> {
  const options = grepOptions(input, context?.cwd)
  if (typeof options === 'string') return errorResult(options)
  if (!(await ripgrepPath())) return grepFallback(options)
  const args = ripgrepArgs(options)

  try {
    // Fast path: content + head_limit without offset → stream and stop after N
    // lines instead of walking the rest of a big repo. files_with_matches keeps
    // the full list because its order (newest first) needs every match.
    if (options.outputMode === 'content' && options.headLimit > 0 && options.offset === 0) {
      const collected: string[] = []
      const controller = new AbortController()
      let timedOut = false
      const timeoutId = setTimeout(() => {
        timedOut = true
        controller.abort()
      }, RIPGREP_TIMEOUT_MS)
      let streamErr: unknown
      try {
        await ripgrepStream(args.slice(0, -1), args[args.length - 1]!, controller.signal, (lines) => {
          for (const line of lines) {
            collected.push(line)
            if (collected.length >= options.headLimit) {
              controller.abort()
              return
            }
          }
        })
      } catch (err) {
        if ((err as Error)?.name !== 'AbortError') streamErr = err
      } finally {
        clearTimeout(timeoutId)
      }
      if (!streamErr) {
        const result = collected.slice(0, options.headLimit).join('\n')
        if (timedOut && !result.trim()) return timeoutError()
        const suffix = timedOut ? `\n[Search timed out after ${RIPGREP_TIMEOUT_MS / 1000}s — partial results above]` : ''
        return textResult((result || 'No matches found.') + suffix)
      }
    }

    let res = await spawnRipgrep(args, RIPGREP_TIMEOUT_MS)
    if (res.code !== null && res.code > 1 && isEagainError(res.stderr)) {
      res = await spawnRipgrep(args, RIPGREP_TIMEOUT_MS, true)
    }
    // rg exits 0 = matches, 1 = no matches; 2 = an error such as a bad regex or
    // a missing path. Report it instead of re-running the search another way.
    if (res.code !== null && res.code > 1 && !res.truncated && !res.timedOut && !res.stdout.trim()) {
      return errorResult(`ripgrep failed (exit ${res.code}): ${res.stderr.trim().slice(0, 2000) || 'no error text'}`)
    }

    let lines = res.stdout.replace(/\r$/gm, '').split('\n')
    if (lines[lines.length - 1] === '') lines.pop()
    // Drop the final line when output was cut off — rg may have been killed
    // mid-line, so the trailing fragment is unreliable.
    if ((res.truncated || res.timedOut) && lines.length > 1) lines.pop()
    if (options.outputMode === 'files_with_matches') lines = await sortByMtime(lines)
    const result = paginate(lines, options.offset, options.headLimit).join('\n')

    if (res.timedOut && !result.trim()) return timeoutError()
    const suffix = [
      res.truncated ? `\n[Output truncated at ${MAX_GREP_BYTES / 1024 / 1024} MB — narrow the path/pattern for full results]` : '',
      res.timedOut ? `\n[Search timed out after ${RIPGREP_TIMEOUT_MS / 1000}s — partial results above; try a narrower path or pattern]` : '',
      res.code !== null && res.code > 1 ? `\n[ripgrep reported errors: ${res.stderr.trim().split('\n')[0]?.slice(0, 300)}]` : '',
    ].join('')
    return textResult((result || 'No matches found.') + suffix)
  } catch {
    return grepFallback(options)
  }
}

function timeoutError(): McpResult {
  return errorResult(
    `Ripgrep search timed out after ${RIPGREP_TIMEOUT_MS / 1000}s with no matches captured. ` +
      `Try narrowing the path or pattern, or pass type/glob to scope the file set.`,
  )
}

/** Reject patterns prone to catastrophic backtracking BEFORE compiling. The JS
 *  regex engine has no timeout, and this fallback runs on the shared exthost
 *  event loop, so a pattern like `(a+)+$` or `(.*a){20}` on one long line would
 *  spin the loop synchronously — a whole-app freeze no timer can break. ripgrep
 *  (the primary path) uses a linear engine and is immune; this guard only bites
 *  the rare no-ripgrep fallback. Heuristic: an unbounded-quantifier group that
 *  is itself quantified (nested quantifier) — the classic ReDoS shape. */
function hasNestedQuantifier(p: string): boolean {
  return /\([^()]*[+*][^()]*\)\s*[+*{]/.test(p)
}

/** Wall-clock budget of the in-process fallback, matching ripgrep's 9 min. */
const FALLBACK_TIMEOUT_MS = 540_000
const FALLBACK_NOTICE =
  '\n[ripgrep is not installed: searched with the built-in fallback. .gitignore rules are not applied; node_modules and VCS directories are skipped.]'

/** Arguments the fallback cannot honour with ripgrep's meaning. */
function unsupportedByFallback(o: GrepOptions): string[] {
  const out: string[] = []
  if (o.fileType) out.push('type')
  if (o.multiline) out.push('multiline')
  if (o.outputMode === 'content' && (o.contextLines || o.afterContext || o.beforeContext)) out.push('-A/-B/-C')
  return out
}

async function grepFallback(o: GrepOptions): Promise<McpResult> {
  const unsupported = unsupportedByFallback(o)
  if (unsupported.length) {
    return errorResult(
      `ripgrep is not installed on this machine, and the built-in fallback search does not support ${unsupported.join(', ')}. ` +
        `Retry without ${unsupported.length > 1 ? 'these arguments' : 'this argument'} (use glob instead of type), or install ripgrep on PATH.`,
    )
  }
  if (hasNestedQuantifier(o.pattern)) {
    return errorResult(
      `Pattern "${o.pattern}" has a nested quantifier that can hang the JS search engine, ` +
        `and ripgrep isn't available on this machine to run it safely. Simplify the pattern ` +
        `(avoid nested quantifiers like (a+)+ / (.*x){n}) or install ripgrep on PATH.`,
    )
  }
  let regex: RegExp
  try {
    regex = new RegExp(o.pattern, o.caseInsensitive ? 'gi' : 'g')
  } catch (e) {
    return errorResult(`Invalid regex pattern: ${e instanceof Error ? e.message : String(e)}`)
  }

  let isFile = false
  try {
    isFile = (await fsp.stat(o.searchPath)).isFile()
  } catch {
    return errorResult(`Path does not exist: ${o.searchPath}`)
  }
  if (isFile) {
    const matches = await matchFile(regex, o.searchPath, o.outputMode, o.showLineNumbers, false)
    const results = matches === null ? [] : o.outputMode === 'content' ? matches : [matches[0]!]
    return fallbackResult(results, o.offset, o.headLimit, false, FALLBACK_TIMEOUT_MS, FALLBACK_NOTICE)
  }
  const walk = await walkFallback(regex, o.searchPath, o.outputMode, o.headLimit + o.offset, FALLBACK_TIMEOUT_MS, o.globs, o.showLineNumbers)
  const results = o.outputMode === 'files_with_matches' ? await sortByMtime(walk.results) : walk.results
  return fallbackResult(results, o.offset, o.headLimit, walk.timedOut, FALLBACK_TIMEOUT_MS, FALLBACK_NOTICE)
}

/** Matches of one file in ripgrep's non-tty format; null when none or unreadable. */
async function matchFile(
  regex: RegExp,
  file: string,
  outputMode: string,
  lineNumbers: boolean,
  withPath: boolean,
): Promise<string[] | null> {
  try {
    const stat = await fsp.stat(file)
    if (stat.size > MAX_READ_BYTES) return null
    const buffer = await fsp.readFile(file)
    // ripgrep skips binary files (NUL byte in the first block).
    if (buffer.subarray(0, 8192).includes(0)) return null
    const lines = buffer.toString('utf8').split('\n')
    const out: string[] = []
    let count = 0
    for (let i = 0; i < lines.length; i++) {
      const line = (lines[i] ?? '').replace(/\r$/, '')
      regex.lastIndex = 0
      if (!regex.test(line)) continue
      count++
      if (outputMode === 'content') {
        const text = line.length > MAX_COLUMNS ? '[Omitted long matching line]' : line
        out.push(`${withPath ? `${file}:` : ''}${lineNumbers ? `${i + 1}:` : ''}${text}`)
      }
    }
    if (count === 0) return null
    if (outputMode === 'files_with_matches') return [file]
    if (outputMode === 'count') return [withPath ? `${file}:${count}` : String(count)]
    return out
  } catch {
    return null
  }
}

/** ripgrep matches a glob without `/` against the file name at any depth. */
function fastGlobPattern(glob: string): string {
  return glob.includes('/') ? glob.replace(/^\//, '') : `**/${glob}`
}

/** Directory walk of the fallback, bounded by `timeoutMs` of wall clock and,
 *  when `limit` > 0, by the number of result entries. */
export async function walkFallback(
  regex: RegExp,
  searchPath: string,
  outputMode: string,
  limit: number,
  timeoutMs: number,
  globs: string[] = [],
  lineNumbers = true,
): Promise<{ results: string[]; timedOut: boolean }> {
  const results: string[] = []
  const deadline = Date.now() + timeoutMs
  let timedOut = false
  const include = globs.filter((g) => !g.startsWith('!')).map(fastGlobPattern)
  const exclude = globs.filter((g) => g.startsWith('!')).map((g) => fastGlobPattern(g.slice(1)))
  const stream = fg.stream(include.length ? include : ['**/*'], {
    cwd: searchPath,
    absolute: true,
    dot: true,
    onlyFiles: true,
    followSymbolicLinks: false,
    suppressErrors: true,
    ignore: [...VCS_DIRECTORIES.map((d) => `**/${d}/**`), '**/node_modules/**', ...exclude],
  })
  try {
    for await (const entry of stream) {
      if (limit > 0 && results.length >= limit) break
      if (Date.now() > deadline) {
        timedOut = true
        break
      }
      const file = path.normalize(entry as string)
      const matches = await matchFile(regex, file, outputMode, lineNumbers, true)
      if (matches) results.push(...matches)
    }
  } finally {
    ;(stream as unknown as { destroy?: () => void }).destroy?.()
  }
  return { results, timedOut }
}

/** Final fallback text; a deadline-cut walk is reported, never passed off as complete. */
export function fallbackResult(
  results: string[],
  offset: number,
  headLimit: number,
  timedOut: boolean,
  timeoutMs: number,
  notice = '',
): McpResult {
  const output = paginate(results, offset, headLimit).join('\n')
  const seconds = Math.round(timeoutMs / 1000)
  if (timedOut && !output.trim()) {
    return errorResult(
      `Search timed out after ${seconds}s with no matches captured; not every file was searched. ` +
        `Try narrowing the path or pattern, or pass glob to scope the file set.`,
    )
  }
  const suffix = timedOut ? `\n[Search timed out after ${seconds}s — partial results above; try a narrower path or pattern]` : ''
  return textResult((output || 'No matches found.') + suffix + notice)
}
