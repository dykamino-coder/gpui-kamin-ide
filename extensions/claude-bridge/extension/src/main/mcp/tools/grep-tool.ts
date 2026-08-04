// Grep MCP tool — wraps `rg` (ripgrep) with stream + buffer modes, head-limit
// fast-path, EAGAIN single-thread retry, and a JS-only fallback when rg is
// missing. Extracted from `file-system.ts` (Sprint 2 / Stage C, C6) — the
// grep family was 377 LOC and dwarfed the rest of the file.

import fs from 'node:fs'
import fsp from 'node:fs/promises'
import path from 'node:path'
import { spawn } from 'node:child_process'
import {
  type McpResult,
  textResult,
  errorResult,
  MAX_GREP_BYTES,
  MAX_READ_BYTES,
} from './fs-shared'

/** Spawn `rg --files` and count matching files (used by Glob's `-l` mode).
 *  Counts lines via byte scan rather than chunk.toString().split — saves on
 *  hot paths where multi-GB repos produce millions of paths. */
export async function ripgrepFileCount(
  args: string[],
  target: string,
  signal: AbortSignal,
): Promise<number> {
  return new Promise<number>((resolve, reject) => {
    if (signal.aborted) return reject(new Error('aborted'))
    const child = spawn('rg', [...args, target], {
      stdio: ['ignore', 'pipe', 'ignore'],
      windowsHide: true,
      signal,
    })

    let lines = 0
    child.stdout.on('data', (chunk: Buffer) => {
      // Faster than chunk.toString().split('\n').length — counts bytes
      // directly, no intermediate strings or arrays.
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
 * fragments are carried across chunk boundaries. Caller can stop early by
 * aborting the signal. Mirrors CLI's `ripGrepStream` (utils/ripgrep.ts) —
 * fast first paint on big repos because we don't wait for full traversal.
 */
export async function ripgrepStream(
  args: string[],
  target: string,
  signal: AbortSignal,
  onLines: (lines: string[]) => void,
): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    if (signal.aborted) return reject(new Error('aborted'))
    const child = spawn('rg', [...args, target], {
      stdio: ['ignore', 'pipe', 'ignore'],
      windowsHide: true,
      signal,
    })

    const stripCR = (l: string): string => (l.endsWith('\r') ? l.slice(0, -1) : l)
    let remainder = ''
    child.stdout.on('data', (chunk: Buffer) => {
      const data = remainder + chunk.toString('utf8')
      const lines = data.split('\n')
      remainder = lines.pop() ?? ''
      if (lines.length) onLines(lines.map(stripCR))
    })

    let settled = false
    child.on('close', (code) => {
      if (settled) return
      // Abort raced close — don't flush a torn tail from a killed process.
      // Abort path settles via the 'error' callback below (AbortError).
      if (signal.aborted) return
      settled = true
      if (code === 0 || code === 1) {
        if (remainder) onLines([stripCR(remainder)])
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
  return new Promise((resolve, reject) => {
    const extraArgs = singleThread ? ['-j', '1'] : []
    // Use bare 'rg' name (not a resolved path) so OS does PATH resolution
    // with NoDefaultCurrentDirectoryInExePath protection (prevents hijacking).
    const child = spawn('rg', [...extraArgs, ...args], {
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    })

    const stdoutChunks: Buffer[] = []
    const stderrChunks: Buffer[] = []
    let totalBytes = 0
    let truncated = false
    let timedOut = false
    let spawnFailed = false
    let killTimer: NodeJS.Timeout | undefined

    const timer = setTimeout(() => {
      timedOut = true
      if (process.platform === 'win32') {
        child.kill()
      } else {
        child.kill('SIGTERM')
        killTimer = setTimeout(() => { if (!child.killed) child.kill('SIGKILL') }, 5000)
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
      spawnFailed = true
      reject(err)
    })

    child.on('close', (code) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      if (killTimer) clearTimeout(killTimer)
      if (spawnFailed) return
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

export async function grepSearch(input: Record<string, unknown>): Promise<McpResult> {
  const pattern = input.pattern as string
  if (!pattern) return errorResult('pattern is required')

  const searchPath = (input.path as string) || process.cwd()
  const outputMode = (input.output_mode as string) || 'files_with_matches'
  const glob = input.glob as string | undefined
  const fileType = input.type as string | undefined
  const caseInsensitive = input['-i'] === true
  const showLineNumbers = input['-n'] !== false
  const multiline = input.multiline === true
  const headLimit = typeof input.head_limit === 'number' ? input.head_limit : 0
  const offset = typeof input.offset === 'number' ? input.offset : 0
  const afterContext = typeof input['-A'] === 'number' ? input['-A'] : undefined
  const beforeContext = typeof input['-B'] === 'number' ? input['-B'] : undefined
  const contextLines = typeof input['-C'] === 'number' ? input['-C'] : (typeof input.context === 'number' ? input.context : undefined)

  const args: string[] = []
  if (outputMode === 'files_with_matches') args.push('--files-with-matches')
  else if (outputMode === 'count') args.push('--count')
  if (caseInsensitive) args.push('-i')
  if (showLineNumbers && outputMode === 'content') args.push('-n')
  if (multiline) args.push('-U', '--multiline-dotall')
  if (outputMode === 'content') {
    if (contextLines !== undefined) args.push('-C', String(contextLines))
    else {
      if (afterContext !== undefined) args.push('-A', String(afterContext))
      if (beforeContext !== undefined) args.push('-B', String(beforeContext))
    }
  }
  if (glob) args.push('--glob', glob)
  if (fileType) args.push('--type', fileType)
  args.push('--', pattern, searchPath)

  try {
    // 9 min for rg — leaves 1-min headroom under the bridge's 10-min MCP
    // cap (HEAVY_TOOL_TIMEOUT_MS in session-manager.ts). Big repos + glob
    // filters legitimately chew up minutes on the first cold run.
    const RIPGREP_TIMEOUT_MS = 540_000

    // Fast path: head_limit + no offset → stream and abort once we have N
    // lines. Saves walking the rest of a multi-GB repo just to slice off
    // 50 results. The buffered variant below only kicks in when the caller
    // wants the full output (sort, count, etc.) or when offset/headLimit
    // require a global view. ripgrepStream signature is (args, target):
    // our `args` ends with `['--', pattern, searchPath]`, so target =
    // searchPath, streamed args = everything before it.
    if (headLimit > 0 && offset === 0) {
      const streamArgs = args.slice(0, -1)
      const target = args[args.length - 1]!
      const collected: string[] = []
      const controller = new AbortController()
      let timedOut = false
      const timeoutId = setTimeout(() => { timedOut = true; controller.abort() }, RIPGREP_TIMEOUT_MS)
      let streamErr: unknown
      try {
        await ripgrepStream(streamArgs, target, controller.signal, lines => {
          for (const line of lines) {
            collected.push(line)
            if (collected.length >= headLimit) { controller.abort(); return }
          }
        })
      } catch (err) {
        // AbortError is expected when we hit the cap or timeout; the
        // `timedOut` flag tells us which one. Anything else falls back
        // to the buffered path.
        if ((err as Error)?.name !== 'AbortError') streamErr = err
      } finally {
        clearTimeout(timeoutId)
      }
      if (!streamErr) {
        const result = collected.slice(0, headLimit).map(l => l.replace(/\r$/, '')).join('\n')
        if (timedOut && (!result || result.trim().length === 0)) {
          const seconds = Math.round(RIPGREP_TIMEOUT_MS / 1000)
          return errorResult(
            `Ripgrep search timed out after ${seconds}s with no matches captured. ` +
            `Try narrowing the path or pattern, or pass --type/--glob to scope the file set.`,
          )
        }
        const suffix = timedOut
          ? `\n[Search timed out after ${Math.round(RIPGREP_TIMEOUT_MS / 1000)}s — partial results above]`
          : ''
        return textResult((result || 'No matches found.') + suffix)
      }
      // Stream path errored — fall through to the buffered path which has
      // EAGAIN retry + grepFallback.
    }

    let res = await spawnRipgrep(args, RIPGREP_TIMEOUT_MS)

    // Retry with single-thread mode on EAGAIN (resource exhaustion in containers/CI)
    if (res.code !== null && res.code !== 0 && res.code !== 1 && isEagainError(res.stderr)) {
      res = await spawnRipgrep(args, RIPGREP_TIMEOUT_MS, true)
    }

    // rg exits 0 = matches, 1 = no matches (both success); other codes = error
    if (res.code !== null && res.code !== 0 && res.code !== 1 && !res.truncated && !res.timedOut) {
      return grepFallback(pattern, searchPath, outputMode, caseInsensitive, headLimit, offset)
    }

    // Strip Windows CR remnants
    let result = res.stdout.replace(/\r$/gm, '')

    // Drop the final line when output was cut off — rg may have been killed
    // mid-line (truncate cap or kill signal), so the trailing fragment is
    // unreliable. Same logic CLI applies in `ripgrep.ts:handleResult`.
    if (res.truncated || res.timedOut) {
      const lines = result.split('\n')
      if (lines.length > 1) result = lines.slice(0, -1).join('\n')
    }

    if (offset > 0) result = result.split('\n').slice(offset).join('\n')
    if (headLimit > 0) result = result.split('\n').slice(0, headLimit).join('\n')

    // Empty + timeout → distinguishable error so the model knows the search
    // didn't complete (vs. genuinely "no matches"). Without this we silently
    // return "No matches found." and the model concludes the term is absent.
    if (res.timedOut && (!result || result.trim().length === 0)) {
      const seconds = Math.round(RIPGREP_TIMEOUT_MS / 1000)
      return errorResult(
        `Ripgrep search timed out after ${seconds}s with no matches captured. ` +
        `The search may have partially matched files but did not complete in time. ` +
        `Try narrowing the path or pattern, or pass --type/--glob to scope the file set.`,
      )
    }

    const suffix = [
      res.truncated ? `\n[Output truncated at ${MAX_GREP_BYTES / 1024 / 1024} MB — narrow the path/pattern for full results]` : '',
      res.timedOut ? `\n[Search timed out after ${Math.round(RIPGREP_TIMEOUT_MS / 1000)}s — partial results above; try a narrower path or pattern]` : '',
    ].join('')

    return textResult((result || 'No matches found.') + suffix)
  } catch {
    return grepFallback(pattern, searchPath, outputMode, caseInsensitive, headLimit, offset)
  }
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

// Async fallback when ripgrep isn't available (streaming readdir)
async function grepFallback(
  pattern: string, searchPath: string, outputMode: string,
  caseInsensitive: boolean, headLimit: number, offset: number,
): Promise<McpResult> {
  if (hasNestedQuantifier(pattern)) {
    return errorResult(
      `Pattern "${pattern}" has a nested quantifier that can hang the JS search engine, ` +
      `and ripgrep isn't available on this machine to run it safely. Simplify the pattern ` +
      `(avoid nested quantifiers like (a+)+ / (.*x){n}) or install ripgrep on PATH.`,
    )
  }
  const flags = caseInsensitive ? 'gi' : 'g'
  let regex: RegExp
  try { regex = new RegExp(pattern, flags) }
  catch (e) { return errorResult(`Invalid regex pattern: ${e instanceof Error ? e.message : String(e)}`) }
  const results: string[] = []

  async function searchDir(dir: string): Promise<void> {
    let entries: fs.Dirent[]
    try {
      entries = await fsp.readdir(dir, { withFileTypes: true })
    } catch {
      return
    }
    for (const entry of entries) {
      if (headLimit > 0 && results.length >= headLimit) return
      if (entry.name.startsWith('.') || entry.name === 'node_modules') continue
      const fullPath = path.join(dir, entry.name)
      if (entry.isDirectory()) {
        await searchDir(fullPath)
      } else if (entry.isFile()) {
        try {
          const stat = await fsp.stat(fullPath)
          if (stat.size > MAX_READ_BYTES) continue
          const content = await fsp.readFile(fullPath, 'utf8')
          const lines = content.split('\n')
          const matchingLines: string[] = []
          for (let i = 0; i < lines.length; i++) {
            const line = lines[i] ?? ''
            if (regex.test(line)) {
              matchingLines.push(outputMode === 'content' ? `${i + 1}:${line}` : line)
            }
            regex.lastIndex = 0
          }
          if (matchingLines.length > 0) {
            if (outputMode === 'files_with_matches') results.push(fullPath)
            else if (outputMode === 'count') results.push(`${fullPath}:${matchingLines.length}`)
            else results.push(`${fullPath}:\n${matchingLines.join('\n')}`)
          }
        } catch {
          // skip unreadable
        }
      }
    }
  }

  try {
    const stat = await fsp.stat(searchPath)
    if (stat.isFile()) {
      const content = await fsp.readFile(searchPath, 'utf8')
      const lines = content.split('\n')
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i] ?? ''
        if (regex.test(line) && outputMode === 'content') results.push(`${i + 1}:${line}`)
        regex.lastIndex = 0
      }
      if (results.length > 0 && outputMode === 'files_with_matches') return textResult(searchPath)
      if (outputMode === 'count') return textResult(`${searchPath}:${results.length}`)
    } else {
      await searchDir(searchPath)
    }
  } catch {
    await searchDir(searchPath)
  }

  let output = results.join('\n')
  if (offset > 0) output = output.split('\n').slice(offset).join('\n')
  if (headLimit > 0) output = output.split('\n').slice(0, headLimit).join('\n')

  return textResult(output || 'No matches found.')
}
