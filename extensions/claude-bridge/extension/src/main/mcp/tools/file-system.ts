import fs from 'node:fs'
import fsp from 'node:fs/promises'
import path from 'node:path'
import fg from 'fast-glob'
import { notebookRead } from './notebook'
import { readFileCached, invalidateFileCache } from '../../utils/file-read-cache'
import {
  type McpResult,
  textResult,
  errorResult,
  MAX_READ_BYTES,
  WARN_READ_BYTES,
  STAT_CONCURRENCY,
  IMAGE_EXTENSIONS,
  isBlockedDevicePath,
} from './fs-shared'

export type { McpResult } from './fs-shared'
// Re-export grep family — callers (executor + tool-registry) import these
// from `./file-system` historically; keep that surface stable.
export { grepSearch, ripgrepStream, ripgrepFileCount } from './grep-tool'

// `fast-glob` is imported statically so Vite's main-process bundler inlines
// it. The previous lazy `require('fast-glob')` saved ~30ms on cold start in
// dev, but `@electron-forge/plugin-vite` drops node_modules from the staged
// app entirely (see `electron/forge.config.ts` afterCopy comment), so the
// dynamic require silently failed in packaged builds with "Cannot find
// module 'fast-glob'" — and the model fell back to listing files via Bash.
// Bundling at the cost of 30ms cold start is the right trade.

// Curly-quote normalization for fuzzy string matching in Edit
const LEFT_SINGLE_CURLY_QUOTE = '\u2018'
const RIGHT_SINGLE_CURLY_QUOTE = '\u2019'
const LEFT_DOUBLE_CURLY_QUOTE = '\u201C'
const RIGHT_DOUBLE_CURLY_QUOTE = '\u201D'

function normalizeQuotes(str: string): string {
  return str
    .replaceAll(LEFT_SINGLE_CURLY_QUOTE, "'")
    .replaceAll(RIGHT_SINGLE_CURLY_QUOTE, "'")
    .replaceAll(LEFT_DOUBLE_CURLY_QUOTE, '"')
    .replaceAll(RIGHT_DOUBLE_CURLY_QUOTE, '"')
}

// Find the actual substring in content that equals searchString after quote normalization.
// Returns the original substring (with its curly quotes) or null.
function findActualString(content: string, searchString: string): string | null {
  if (content.includes(searchString)) return searchString
  const normalizedContent = normalizeQuotes(content)
  const normalizedSearch = normalizeQuotes(searchString)
  const idx = normalizedContent.indexOf(normalizedSearch)
  if (idx === -1) return null
  return content.substring(idx, idx + searchString.length)
}

// Decide if a quote character at the given position is opening or closing —
// opening when preceded by whitespace, start-of-string, or opening punctuation.
// Closing otherwise. Mirrors CLI's `isOpeningContext` heuristic.
function isOpeningContext(chars: string[], index: number): boolean {
  if (index === 0) return true
  const prev = chars[index - 1]
  return (
    prev === ' ' || prev === '\t' || prev === '\n' || prev === '\r' ||
    prev === '(' || prev === '[' || prev === '{' ||
    prev === '—' || prev === '–' // em/en dash
  )
}

// Replace ASCII " with curly variants based on open/close context.
function applyCurlyDoubleQuotes(str: string): string {
  const chars = [...str]
  const out: string[] = []
  for (let i = 0; i < chars.length; i++) {
    if (chars[i] === '"') {
      out.push(isOpeningContext(chars, i) ? LEFT_DOUBLE_CURLY_QUOTE : RIGHT_DOUBLE_CURLY_QUOTE)
    } else {
      out.push(chars[i]!)
    }
  }
  return out.join('')
}

// Replace ASCII ' with curly variants. Treats apostrophes inside contractions
// (`don't`, `it's`) as right-single-curly to match standard typography.
function applyCurlySingleQuotes(str: string): string {
  const chars = [...str]
  const out: string[] = []
  for (let i = 0; i < chars.length; i++) {
    if (chars[i] === "'") {
      const prev = i > 0 ? chars[i - 1] : undefined
      const next = i < chars.length - 1 ? chars[i + 1] : undefined
      const prevIsLetter = prev !== undefined && /\p{L}/u.test(prev)
      const nextIsLetter = next !== undefined && /\p{L}/u.test(next)
      if (prevIsLetter && nextIsLetter) {
        out.push(RIGHT_SINGLE_CURLY_QUOTE)
      } else {
        out.push(isOpeningContext(chars, i) ? LEFT_SINGLE_CURLY_QUOTE : RIGHT_SINGLE_CURLY_QUOTE)
      }
    } else {
      out.push(chars[i]!)
    }
  }
  return out.join('')
}

// When the fuzzy curly-quote matcher found `actualOldString` (containing
// curly typography) but the model supplied straight quotes in old_string,
// echo the same curly typography into new_string so the edit preserves the
// file's style. Without this we silently downgrade ‘ ’ “ ” → ' ' " ".
function preserveQuoteStyle(
  oldString: string,
  actualOldString: string,
  newString: string,
): string {
  if (oldString === actualOldString) return newString
  const hasDouble =
    actualOldString.includes(LEFT_DOUBLE_CURLY_QUOTE) ||
    actualOldString.includes(RIGHT_DOUBLE_CURLY_QUOTE)
  const hasSingle =
    actualOldString.includes(LEFT_SINGLE_CURLY_QUOTE) ||
    actualOldString.includes(RIGHT_SINGLE_CURLY_QUOTE)
  if (!hasDouble && !hasSingle) return newString
  let out = newString
  if (hasDouble) out = applyCurlyDoubleQuotes(out)
  if (hasSingle) out = applyCurlySingleQuotes(out)
  return out
}

// ── Read ────────────────────────────────────────────────────────────────

export async function readFile(input: Record<string, unknown>): Promise<McpResult> {
  const filePath = input.file_path as string
  if (!filePath) return errorResult('file_path is required')
  if (isBlockedDevicePath(filePath)) return errorResult(`Refusing to read blocking device path: ${filePath}`)

  try {
    const ext = path.extname(filePath).toLowerCase()

    if (ext === '.pdf') {
      return textResult(
        'PDF reading requires the pdf-parse package. Use `Bash` tool with `pdftotext` or similar to extract PDF text.'
      )
    }

    if (ext === '.ipynb') {
      return notebookRead({ file_path: filePath })
    }

    const stat = await fsp.stat(filePath)
    if (!stat.isFile()) return errorResult(`Not a file: ${filePath}`)
    if (stat.size > MAX_READ_BYTES) {
      return errorResult(
        `File too large (${Math.round(stat.size / 1024 / 1024)} MB > ${MAX_READ_BYTES / 1024 / 1024} MB limit). ` +
        `Use offset+limit to read a range, or use Bash with head/tail/sed.`
      )
    }

    if (IMAGE_EXTENSIONS.has(ext)) {
      const raw = await fsp.readFile(filePath)
      const base64 = raw.toString('base64')
      const mimeType = (ext === '.jpg' || ext === '.jpeg') ? 'image/jpeg'
        : `image/${ext.slice(1)}` as 'image/png' | 'image/gif' | 'image/webp'
      return { content: [{ type: 'image', data: base64, mimeType }] }
    }

    const raw = await readFileCached(filePath)

    if (raw.length === 0 || raw.trim().length === 0) {
      return textResult('<system-reminder>Warning: File exists but has empty contents.</system-reminder>')
    }

    const allLines = raw.split('\n')
    const offset = typeof input.offset === 'number' ? Math.max(0, input.offset - 1) : 0
    const limit = typeof input.limit === 'number' ? input.limit : Math.min(allLines.length, 2000)
    const slice = allLines.slice(offset, offset + limit)

    const MAX_LINE_LENGTH = 2000
    const formatted = slice
      .map((line, i) => {
        const lineNum = offset + i + 1
        const truncated = line.length > MAX_LINE_LENGTH
          ? line.slice(0, MAX_LINE_LENGTH) + '... [truncated]'
          : line
        return `${String(lineNum).padStart(6, ' ')}\t${truncated}`
      })
      .join('\n')

    let suffix = ''
    if (stat.size > WARN_READ_BYTES && input.offset === undefined && input.limit === undefined) {
      suffix = `\n\n<system-reminder>File is ${Math.round(stat.size / 1024)} KB (${allLines.length} lines). ` +
               `Consider using offset/limit for targeted reads.</system-reminder>`
    }

    return textResult(formatted + suffix)
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    if (msg.includes('ENOENT')) return errorResult(`File not found: ${filePath}`)
    return errorResult(msg)
  }
}

// ── Write ───────────────────────────────────────────────────────────────

export async function writeFile(input: Record<string, unknown>): Promise<McpResult> {
  const filePath = input.file_path as string
  const content = input.content as string
  if (!filePath) return errorResult('file_path is required')
  if (typeof content !== 'string') return errorResult('content is required')

  try {
    const dir = path.dirname(filePath)
    await fsp.mkdir(dir, { recursive: true })
    await fsp.writeFile(filePath, content, 'utf8')
    // Drop the cached copy: Read calls between this Write and the next
    // mtime tick would otherwise return stale content (mtime resolution
    // is filesystem-dependent and can collapse same-second writes).
    invalidateFileCache(filePath)
    return textResult(`Successfully wrote ${Buffer.byteLength(content, 'utf8')} bytes to ${filePath}`)
  } catch (err: unknown) {
    return errorResult(err instanceof Error ? err.message : String(err))
  }
}

// ── Edit ────────────────────────────────────────────────────────────────

function detectLineEnding(content: string): 'CRLF' | 'LF' {
  let crlfCount = 0
  let lfCount = 0
  let idx = 0
  while (idx < content.length) {
    const next = content.indexOf('\n', idx)
    if (next === -1) break
    if (next > 0 && content.charCodeAt(next - 1) === 13) crlfCount++
    else lfCount++
    idx = next + 1
  }
  return crlfCount > lfCount ? 'CRLF' : 'LF'
}

export async function editFile(input: Record<string, unknown>): Promise<McpResult> {
  const filePath = input.file_path as string
  const oldString = input.old_string as string
  const newString = input.new_string as string
  const replaceAll = input.replace_all === true

  if (!filePath) return errorResult('file_path is required')
  if (typeof oldString !== 'string') return errorResult('old_string is required')
  if (typeof newString !== 'string') return errorResult('new_string is required')
  if (oldString === newString) return errorResult('Original and edited file match exactly. Failed to apply edit.')

  try {
    const stat = await fsp.stat(filePath)
    if (stat.size > MAX_READ_BYTES) {
      return errorResult(
        `File too large to edit (${Math.round(stat.size / 1024 / 1024)} MB > ${MAX_READ_BYTES / 1024 / 1024} MB limit)`
      )
    }

    const rawContent = await readFileCached(filePath)
    const lineEnding = detectLineEnding(rawContent)

    const content = rawContent.replaceAll('\r\n', '\n')
    const normalizedOld = oldString.replaceAll('\r\n', '\n')
    const normalizedNew = newString.replaceAll('\r\n', '\n')

    let effectiveOld = normalizedOld
    if (normalizedNew === '' && !normalizedOld.endsWith('\n') && content.includes(normalizedOld + '\n')) {
      effectiveOld = normalizedOld + '\n'
    }

    // Fuzzy match via curly-quote normalization — LLM often outputs straight
    // quotes while the file contains typographic curly quotes ("") or ('').
    if (!content.includes(effectiveOld)) {
      const actual = findActualString(content, effectiveOld)
      if (actual !== null) effectiveOld = actual
    }

    // If fuzzy match swapped curly→ASCII, mirror the curly typography back
    // into new_string so the edit doesn't silently downgrade the file's
    // quote style. No-op when the match was exact.
    const effectiveNew = preserveQuoteStyle(normalizedOld, effectiveOld, normalizedNew)

    // Pass new_string through a function callback so `$&`, `$1`, `$$`
    // sequences in user content are not interpreted as String.replace
    // backreferences. CLI does the same for the same reason.
    let result: string
    if (replaceAll) {
      result = content.replaceAll(effectiveOld, () => effectiveNew)
    } else {
      result = content.replace(effectiveOld, () => effectiveNew)
    }

    if (result === content) {
      return errorResult('String not found in file. Failed to apply edit.')
    }

    if (lineEnding === 'CRLF') {
      result = result.replaceAll('\n', '\r\n')
    }

    await fsp.writeFile(filePath, result, 'utf8')
    invalidateFileCache(filePath)
    return textResult(`The file ${filePath} has been updated successfully.`)
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    if (msg.includes('ENOENT')) return errorResult(`File not found: ${filePath}`)
    return errorResult(msg)
  }
}

// ── Glob ────────────────────────────────────────────────────────────────

async function batchedStat(files: string[]): Promise<Array<{ file: string; mtime: number }>> {
  const out: Array<{ file: string; mtime: number }> = []
  for (let i = 0; i < files.length; i += STAT_CONCURRENCY) {
    const batch = files.slice(i, i + STAT_CONCURRENCY)
    const stats = await Promise.all(
      batch.map(async (f) => {
        try {
          const s = await fsp.stat(f)
          return { file: f, mtime: s.mtimeMs }
        } catch {
          return { file: f, mtime: 0 }
        }
      }),
    )
    out.push(...stats)
  }
  return out
}

const MAX_GLOB_RESULTS = 10_000

/** A filesystem root (`/`, `C:\`, `C:/`) — an unbounded glob from here walks the
 *  whole drive. Real globs are project-scoped. */
function isFsRoot(p: string): boolean {
  return /^([a-zA-Z]:[\\/]?|[\\/])$/.test(p.trim())
}

export async function globSearch(input: Record<string, unknown>): Promise<McpResult> {
  const pattern = input.pattern as string
  if (!pattern) return errorResult('pattern is required')

  const basePath = (input.path as string) || process.cwd()

  // Refuse an unbounded walk from a filesystem root — `Glob("**", "C:\\")` would
  // enumerate + stat millions of entries (minutes of work, hundreds of MB).
  if (isFsRoot(basePath) && /^\*\*?(\/\*+)?$/.test(pattern.trim())) {
    return errorResult('Refusing an unbounded glob from a filesystem root — scope `path` to a project or narrow the pattern.')
  }

  try {
    // Stream + cap so a broad pattern can't materialise a multi-million-entry
    // array in memory; we stop at MAX_GLOB_RESULTS and tell the model to narrow.
    const files: string[] = []
    let capped = false
    const stream = fg.stream(pattern, {
      cwd: basePath,
      absolute: true,
      dot: true,
      onlyFiles: true,
      followSymbolicLinks: true,
      ignore: ['**/.git/**', '**/node_modules/**', '**/.next/**', '**/dist/**', '**/build/**', '**/.vite/**'],
    })
    for await (const entry of stream) {
      files.push(entry as string)
      if (files.length >= MAX_GLOB_RESULTS) { capped = true; break }
    }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if (capped) (stream as any).destroy?.()

    const withStats = await batchedStat(files)
    withStats.sort((a, b) => b.mtime - a.mtime)

    const result = withStats.map((s) => s.file).join('\n')
    const suffix = capped ? `\n[Showing first ${MAX_GLOB_RESULTS} matches — narrow the pattern/path for the rest]` : ''
    return textResult((result || 'No files matched the pattern.') + suffix)
  } catch (err: unknown) {
    return errorResult(err instanceof Error ? err.message : String(err))
  }
}

// ── Grep (async spawn + bounded stdout) ─────────────────────────────────

/**
 * Stream `rg --files` and count matching lines without buffering. Each
 * stdout chunk's `\n` bytes are tallied directly; peak memory is one
 * stream chunk (~64KB) regardless of repo size. The buffered variant
 * materialised the full path list (16MB on a 247k-file mono-repo) just
 * to read `.length`. Aborting the supplied signal kills the child.
 */
