import fs from 'node:fs'
import fsp from 'node:fs/promises'
import path from 'node:path'
import { notebookRead } from './notebook'
import { readTextCached, invalidateFileCache } from '../../utils/file-read-cache'
import { detectEncoding, encodeText, type TextEncoding } from './text-codec'
import { globQuery, listGlob, MAX_GLOB_LISTED, newestFirst } from './glob-tool'
import { resolveToolPath, resolveWorkingDirectory } from '../../utils/working-directory'
import type { ToolCallContext } from '../executor'
import {
  type McpResult,
  textResult,
  errorResult,
  MAX_READ_BYTES,
  WARN_READ_BYTES,
  IMAGE_EXTENSIONS,
  isBlockedDevicePath,
} from './fs-shared'

export type { McpResult } from './fs-shared'
// Re-export grep family — callers (executor + tool-registry) import these
// from `./file-system` historically; keep that surface stable.
export { grepSearch, ripgrepStream, ripgrepFileCount } from './grep-tool'


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

const MAX_LINE_LENGTH = 2000

export async function readFile(input: Record<string, unknown>, context?: ToolCallContext): Promise<McpResult> {
  if (!input.file_path) return errorResult('file_path is required')
  const filePath = resolveToolPath(input.file_path as string, context?.cwd)
  if (isBlockedDevicePath(filePath)) return errorResult(`Refusing to read blocking device path: ${filePath}`)

  try {
    const ext = path.extname(filePath).toLowerCase()

    if (ext === '.pdf') {
      return errorResult(
        'PDF reading is not supported by this Read tool. Use the Bash tool with `pdftotext` or similar to extract PDF text.',
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
          `Use offset+limit to read a range, or use Bash with head/tail/sed.`,
      )
    }

    if (IMAGE_EXTENSIONS.has(ext)) {
      const raw = await fsp.readFile(filePath)
      const base64 = raw.toString('base64')
      const mimeType =
        ext === '.jpg' || ext === '.jpeg' ? 'image/jpeg' : (`image/${ext.slice(1)}` as 'image/png' | 'image/gif' | 'image/webp')
      return { content: [{ type: 'image', data: base64, mimeType }] }
    }

    const file = await readTextCached(filePath, stat)
    if (file.binary) {
      return errorResult(
        `This tool cannot read binary files. ${filePath} appears to be binary (it contains NUL bytes). ` +
          `Use Bash (for example \`xxd | head\` or \`file\`) to inspect it.`,
      )
    }
    const raw = file.content

    if (raw.length === 0 || raw.trim().length === 0) {
      return textResult('<system-reminder>Warning: File exists but has empty contents.</system-reminder>')
    }

    const allLines = raw.split('\n')
    if (allLines.length > 1 && allLines[allLines.length - 1] === '') allLines.pop()
    const offset = typeof input.offset === 'number' ? Math.max(0, Math.floor(input.offset) - 1) : 0
    const limit = typeof input.limit === 'number' && input.limit > 0 ? Math.floor(input.limit) : 2000
    if (offset >= allLines.length) {
      return textResult(
        `<system-reminder>Warning: the file exists but is shorter than the provided offset (${offset + 1}). ` +
          `The file has ${allLines.length} lines.</system-reminder>`,
      )
    }
    const slice = allLines.slice(offset, offset + limit)

    const formatted = slice
      .map((line, i) => {
        // CRLF files: the line ending is not content; Edit restores it.
        const text = line.endsWith('\r') ? line.slice(0, -1) : line
        const shown = text.length > MAX_LINE_LENGTH ? text.slice(0, MAX_LINE_LENGTH) + '... [truncated]' : text
        return `${String(offset + i + 1).padStart(6, ' ')}\t${shown}`
      })
      .join('\n')

    let suffix = ''
    const end = offset + slice.length
    if (input.limit === undefined && end < allLines.length) {
      suffix =
        `\n\n<system-reminder>Showing lines ${offset + 1}-${end} of ${allLines.length}. ` +
        `Use offset/limit to read the rest.</system-reminder>`
    } else if (stat.size > WARN_READ_BYTES && input.offset === undefined && input.limit === undefined) {
      suffix =
        `\n\n<system-reminder>File is ${Math.round(stat.size / 1024)} KB (${allLines.length} lines). ` +
        `Consider using offset/limit for targeted reads.</system-reminder>`
    }

    return textResult(formatted + suffix)
  } catch (err: unknown) {
    return fsError(err, filePath)
  }
}

function fsError(err: unknown, filePath: string): McpResult {
  const code = (err as NodeJS.ErrnoException)?.code
  if (code === 'ENOENT') return errorResult(`File does not exist: ${filePath}`)
  if (code === 'EISDIR') return errorResult(`Path is a directory, not a file: ${filePath}`)
  if (code === 'EACCES' || code === 'EPERM') return errorResult(`Permission denied: ${filePath}`)
  if (code === 'EBUSY') return errorResult(`File is locked by another process: ${filePath}`)
  return errorResult(err instanceof Error ? err.message : String(err))
}

// ── Write ───────────────────────────────────────────────────────────────

/** Encoding of an existing file, so a rewrite keeps its BOM / UTF-16. */
async function existingEncoding(filePath: string): Promise<TextEncoding> {
  try {
    const handle = await fsp.open(filePath, 'r')
    try {
      const head = Buffer.alloc(3)
      const { bytesRead } = await handle.read(head, 0, 3, 0)
      return detectEncoding(head.subarray(0, bytesRead))
    } finally {
      await handle.close()
    }
  } catch {
    return 'utf8'
  }
}

export async function writeFile(input: Record<string, unknown>, context?: ToolCallContext): Promise<McpResult> {
  const content = input.content as string
  if (!input.file_path) return errorResult('file_path is required')
  const filePath = resolveToolPath(input.file_path as string, context?.cwd)
  if (typeof content !== 'string') return errorResult('content is required')

  try {
    await fsp.mkdir(path.dirname(filePath), { recursive: true })
    const bytes = encodeText(content, await existingEncoding(filePath))
    await fsp.writeFile(filePath, bytes)
    // Drop the cached copy: Read calls between this Write and the next
    // mtime tick would otherwise return stale content (mtime resolution
    // is filesystem-dependent and can collapse same-second writes).
    invalidateFileCache(filePath)
    return textResult(`Successfully wrote ${bytes.length} bytes to ${filePath}`)
  } catch (err: unknown) {
    return fsError(err, filePath)
  }
}

// ── Edit ────────────────────────────────────────────────────────────────

function isCrlfFile(content: string): boolean {
  let crlf = 0
  let lf = 0
  for (let idx = content.indexOf('\n'); idx !== -1; idx = content.indexOf('\n', idx + 1)) {
    if (idx > 0 && content.charCodeAt(idx - 1) === 13) crlf++
    else lf++
  }
  return crlf > lf
}

function countOccurrences(content: string, needle: string): number {
  let count = 0
  for (let idx = content.indexOf(needle); idx !== -1; idx = content.indexOf(needle, idx + needle.length)) count++
  return count
}

/** The exact text of the file that `oldString` denotes: verbatim, with the
 *  file's CRLF line endings, or with the file's curly quotes. */
function locate(content: string, oldString: string, crlf: boolean): { old: string; eol: '\n' | '\r\n' } | null {
  const lf = oldString.replaceAll('\r\n', '\n')
  const variants: Array<[string, '\n' | '\r\n']> = crlf
    ? [[lf.replaceAll('\n', '\r\n'), '\r\n'], [oldString, '\r\n'], [lf, '\n']]
    : [[oldString, '\n'], [lf, '\n'], [lf.replaceAll('\n', '\r\n'), '\r\n']]
  for (const [candidate, eol] of variants) {
    if (content.includes(candidate)) return { old: candidate, eol }
  }
  for (const [candidate, eol] of variants) {
    const actual = findActualString(content, candidate)
    if (actual !== null) return { old: actual, eol }
  }
  return null
}

export async function editFile(input: Record<string, unknown>, context?: ToolCallContext): Promise<McpResult> {
  const filePath = input.file_path ? resolveToolPath(input.file_path as string, context?.cwd) : ''
  const oldString = input.old_string as string
  const newString = input.new_string as string
  const replaceAll = input.replace_all === true

  if (!filePath) return errorResult('file_path is required')
  if (typeof oldString !== 'string') return errorResult('old_string is required')
  if (typeof newString !== 'string') return errorResult('new_string is required')
  if (oldString === newString) {
    return errorResult('No changes to make: old_string and new_string are exactly the same.')
  }

  try {
    let stat: fs.Stats
    try {
      stat = await fsp.stat(filePath)
    } catch (err) {
      // Native Edit: an empty old_string creates a new file.
      if ((err as NodeJS.ErrnoException)?.code === 'ENOENT' && oldString === '') {
        await fsp.mkdir(path.dirname(filePath), { recursive: true })
        await fsp.writeFile(filePath, newString, 'utf8')
        invalidateFileCache(filePath)
        return textResult(`The file ${filePath} has been created successfully.`)
      }
      throw err
    }
    if (stat.size > MAX_READ_BYTES) {
      return errorResult(
        `File too large to edit (${Math.round(stat.size / 1024 / 1024)} MB > ${MAX_READ_BYTES / 1024 / 1024} MB limit)`,
      )
    }

    const file = await readTextCached(filePath, stat)
    if (file.binary) return errorResult(`Cannot edit a binary file: ${filePath}`)
    const content = file.content

    let result: string
    if (oldString === '') {
      if (content.trim() !== '') return errorResult('Cannot create new file - file already exists.')
      result = newString
    } else {
      const crlf = isCrlfFile(content)
      const found = locate(content, oldString, crlf)
      if (!found) return errorResult(`String to replace not found in file.\nString: ${oldString}`)
      let { old } = found
      const toEol = (s: string): string => {
        const lf = s.replaceAll('\r\n', '\n')
        return found.eol === '\r\n' ? lf.replaceAll('\n', '\r\n') : lf
      }
      // Deleting a whole line also removes its line ending (native parity).
      if (newString === '' && !old.endsWith('\n') && content.includes(old + found.eol)) {
        if (countOccurrences(content, old) === countOccurrences(content, old + found.eol)) old += found.eol
      }
      const matches = countOccurrences(content, old)
      if (matches > 1 && !replaceAll) {
        return errorResult(
          `Found ${matches} matches of the string to replace, but replace_all is false. To replace all occurrences, ` +
            `set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance.\n` +
            `String: ${oldString}`,
        )
      }
      // If the curly-quote matcher found the text, mirror its typography into
      // new_string so the edit does not downgrade the file's quote style.
      const replacement = preserveQuoteStyle(toEol(oldString), old, toEol(newString))
      // A function replacement keeps `$&`, `$1`, `$$` in new_string literal.
      result = replaceAll ? content.replaceAll(old, () => replacement) : content.replace(old, () => replacement)
    }

    await fsp.writeFile(filePath, encodeText(result, file.encoding))
    invalidateFileCache(filePath)
    return textResult(`The file ${filePath} has been updated successfully.`)
  } catch (err: unknown) {
    return fsError(err, filePath)
  }
}

// ── Glob ────────────────────────────────────────────────────────────────

const GLOB_SHOWN = 100

/** A filesystem root (`/`, `C:\`, `C:/`) — an unbounded glob from here walks the
 *  whole drive. Real globs are project-scoped. */
function isFsRoot(p: string): boolean {
  return /^([a-zA-Z]:[\\/]?|[\\/])$/.test(p.trim())
}

export async function globSearch(input: Record<string, unknown>, context?: ToolCallContext): Promise<McpResult> {
  const pattern = input.pattern as string
  if (!pattern) return errorResult('pattern is required')

  const basePath = input.path
    ? resolveToolPath(input.path as string, context?.cwd)
    : resolveWorkingDirectory(context?.cwd)
  const query = globQuery(pattern, basePath)

  // Refuse an unbounded walk from a filesystem root — `Glob("**", "C:\\")` would
  // enumerate + stat millions of entries (minutes of work, hundreds of MB).
  if (isFsRoot(query.base) && /^\/?(\*\*?\/?)+\*?$/.test(query.pattern.trim())) {
    return errorResult('Refusing an unbounded glob from a filesystem root — scope `path` to a project or narrow the pattern.')
  }
  try {
    if (!(await fsp.stat(query.base)).isDirectory()) return errorResult(`Path is not a directory: ${query.base}`)
  } catch (err) {
    return fsError(err, query.base)
  }

  try {
    const { files, capped } = await listGlob(query, MAX_GLOB_LISTED, context?.signal)
    const sorted = await newestFirst(files)

    // Like native Glob, return at most GLOB_SHOWN paths, but newest first and
    // with the number of the rest, so a broad pattern costs ~100 lines of
    // context instead of thousands.
    const shown = sorted.slice(0, GLOB_SHOWN)
    const total = capped ? `more than ${MAX_GLOB_LISTED}` : String(sorted.length)
    const suffix =
      sorted.length > GLOB_SHOWN
        ? capped
          ? `\n(Showing the ${GLOB_SHOWN} most recently modified of the first ${MAX_GLOB_LISTED} matching files listed; more exist. Use a more specific pattern or path.)`
          : `\n(Showing the ${GLOB_SHOWN} most recently modified of ${total} matching files. Use a more specific pattern or path for the rest.)`
        : ''
    return textResult((shown.join('\n') || 'No files matched the pattern.') + suffix)
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
