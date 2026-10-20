// project_session_search: literal or regex search over every transcript of the
// caller's project (optionally their subagents), paged by a continuation cursor
// and bounded per call by results, output characters and scanned bytes.
//
// Order is conversation start time, newest first. The start time of a
// transcript never changes, so a cursor keyed by transcript stays valid while
// files grow; conversations started after the first page are not revisited.

import fs from 'node:fs'
import path from 'node:path'
import {
  ArgError,
  boolArg,
  decodeCursor,
  encodeCursor,
  intArg,
  kindsArg,
  maxCharsArg,
  strArg,
  timeArg,
  type Cursor,
} from './args'
import { headTail, isLineStart, openTranscript, readLines } from './line-reader'
import { redact } from './redact'
import { KINDS, classify, inTimeRange, parseEntry, searchableText, timestampOf, type Kind } from './render'
import {
  isPlainFile,
  listTranscripts,
  resolveScope,
  subagentDir,
  validConversationId,
  type CallerContext,
} from './scope'
import type { ToolOutput } from './types'

/** Bytes of transcript scanned per call before returning a cursor. */
export const SCAN_BUDGET_BYTES = 256 * 1024 * 1024
const MAX_PATTERN = 256
const MAX_TEXT_PER_ENTRY = 2 * 1024 * 1024
const SNIPPETS_PER_ENTRY = 3
const MAX_COUNTED = 1000

interface SearchTarget {
  key: string
  conversationId: string
  agentId: string | null
  file: string
  started: string
}

/**
 * Patterns run on the shared server's event loop, so constructs with
 * catastrophic backtracking are refused instead of timed out: backreferences
 * and any quantified group that itself contains a quantifier or alternation.
 */
export function compileSearch(query: string, isRegex: boolean, caseSensitive: boolean): RegExp {
  const flags = caseSensitive ? 'g' : 'gi'
  if (!isRegex) return new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), flags)
  if (query.length > MAX_PATTERN) throw new ArgError(`regex longer than ${MAX_PATTERN} chars`)
  if (/\\[1-9]|\\k</.test(query)) throw new ArgError('regex backreferences are not supported')
  const stack: Array<{ risky: boolean }> = []
  let lastClosedRisky = false
  for (let i = 0; i < query.length; i++) {
    const ch = query[i]!
    if (ch === '\\') {
      i++
      lastClosedRisky = false
      continue
    }
    if (ch === '[') {
      const close = query.indexOf(']', i + 1)
      i = close === -1 ? query.length : close
      lastClosedRisky = false
      continue
    }
    const quant = ch === '*' || ch === '+' || ch === '{' || ch === '?'
    if (quant && lastClosedRisky && ch !== '?') {
      throw new ArgError(
        'regex with a repeated group that itself repeats or alternates is not supported (backtracking risk)',
      )
    }
    if (ch === '(') stack.push({ risky: false })
    else if (ch === ')') {
      const g = stack.pop()
      lastClosedRisky = !!g?.risky
      if (g?.risky && stack.length > 0) stack[stack.length - 1]!.risky = true
      continue
    } else if ((ch === '*' || ch === '+' || ch === '{' || ch === '|') && stack.length > 0) {
      stack[stack.length - 1]!.risky = true
    }
    lastClosedRisky = false
  }
  try {
    return new RegExp(query, flags)
  } catch (e) {
    throw new ArgError(`invalid regex: ${e instanceof Error ? e.message : String(e)}`)
  }
}

async function startedAt(file: string): Promise<string> {
  try {
    const fh = await openTranscript(file)
    try {
      const { head } = await headTail(fh, 16 * 1024)
      for (const line of head) {
        const t = timestampOf(parseEntry(line) ?? {})
        if (t) return t
      }
    } finally {
      await fh.close()
    }
  } catch {
    /* unreadable — sorts last */
  }
  return ''
}

async function targets(ctx: CallerContext, ids: Set<string> | null, withSubagents: boolean): Promise<SearchTarget[]> {
  const scope = resolveScope(ctx)
  const refs = listTranscripts(ctx, scope, true).filter((r) => !ids || ids.has(r.conversationId))
  const out: SearchTarget[] = []
  const withStart = await Promise.all(refs.map(async (ref) => ({ ref, started: await startedAt(ref.file) })))
  withStart.sort((a, b) =>
    a.started === b.started ? a.ref.conversationId.localeCompare(b.ref.conversationId) : a.started < b.started ? 1 : -1,
  )
  for (const { ref, started } of withStart) {
    out.push({ key: ref.conversationId, conversationId: ref.conversationId, agentId: null, file: ref.file, started })
    const dir = withSubagents ? subagentDir(ref) : null
    if (!dir) continue
    for (const name of fs.readdirSync(dir).sort()) {
      const m = /^agent-([A-Za-z0-9_-]{1,128})\.jsonl$/.exec(name)
      const file = path.join(dir, name)
      if (m && isPlainFile(file)) {
        out.push({
          key: `${ref.conversationId}/${m[1]}`,
          conversationId: ref.conversationId,
          agentId: m[1]!,
          file,
          started,
        })
      }
    }
  }
  return out
}

function snippet(text: string, at: number, len: number, context: number): string {
  const from = Math.max(0, at - context)
  const to = Math.min(text.length, at + len + context)
  return (from > 0 ? '…' : '') + text.slice(from, to).replace(/\s+/g, ' ') + (to < text.length ? '…' : '')
}

export async function searchSessions(ctx: CallerContext, args: Record<string, unknown>): Promise<ToolOutput> {
  const query = strArg(args.query, 'query', 1000)
  if (!query) throw new ArgError('query is required')
  const isRegex = boolArg(args.regex, 'regex', false)
  const caseSensitive = boolArg(args.case_sensitive, 'case_sensitive', false)
  const re = compileSearch(query, isRegex, caseSensitive)
  const kinds: Set<Kind> = kindsArg(args.kinds) ?? new Set(KINDS.filter((k) => k !== 'other'))
  const since = timeArg(args.since, 'since')
  const until = timeArg(args.until, 'until')
  const maxResults = intArg(args.max_results, 'max_results', 30, 1, 200)
  const context = intArg(args.context_chars, 'context_chars', 120, 0, 1000)
  const maxChars = maxCharsArg(args.max_chars)
  const withSubagents = boolArg(args.include_subagents, 'include_subagents', false)
  let ids: Set<string> | null = null
  if (args.conversation_ids !== undefined && args.conversation_ids !== null) {
    if (!Array.isArray(args.conversation_ids) || args.conversation_ids.length > 200) {
      throw new ArgError('conversation_ids must be a list of at most 200 ids')
    }
    ids = new Set(args.conversation_ids.filter(validConversationId))
  }
  const cursor: Cursor | null = decodeCursor(args.cursor)

  // Literal queries skip JSON.parse for lines that cannot contain the needle.
  // The needle is compared in its JSON-escaped form, as it appears on disk.
  const escaped = isRegex ? null : JSON.stringify(query).slice(1, -1)
  const prefilter = escaped === null ? null : caseSensitive ? escaped : escaped.toLowerCase()

  const list = await targets(ctx, ids, withSubagents)
  let ti = 0
  if (cursor) {
    ti = list.findIndex((t) => t.key === cursor.k)
    if (ti === -1) throw new ArgError('cursor refers to a transcript that is no longer in this project')
  }
  let out = ''
  let results = 0
  let scanned = 0
  let redactions = 0
  let next: Cursor | null = null
  const budget = maxChars - 800
  scan: for (; ti < list.length; ti++) {
    const t = list[ti]!
    const fh = await openTranscript(t.file)
    try {
      let startByte = 0
      let startIndex = 0
      if (cursor && t.key === cursor.k) {
        const size = (await fh.stat()).size
        // A cursor past the last (unterminated) line means this transcript is done.
        if (cursor.b < size && !(await isLineStart(fh, cursor.b))) {
          throw new ArgError('cursor does not match this transcript')
        }
        startByte = Math.min(cursor.b, size)
        startIndex = cursor.i
      }
      for await (const line of readLines(fh, startByte, startIndex)) {
        scanned += line.end - line.start + 1
        const after: Cursor = { k: t.key, i: line.index + 1, b: line.end + 1, c: 0 }
        if (
          line.text !== null &&
          (prefilter === null || (caseSensitive ? line.text : line.text.toLowerCase()).includes(prefilter))
        ) {
          const entry = parseEntry(line.text)
          const kind = entry ? classify(entry) : null
          if (entry && kind && kinds.has(kind) && inTimeRange(entry, since, until)) {
            const text = searchableText(entry, kind).slice(0, MAX_TEXT_PER_ENTRY)
            re.lastIndex = 0
            const snippets: string[] = []
            let count = 0
            for (let m = re.exec(text); m && count < MAX_COUNTED; m = re.exec(text)) {
              count++
              if (snippets.length < SNIPPETS_PER_ENTRY) snippets.push(snippet(text, m.index, m[0].length, context))
              if (m[0].length === 0) re.lastIndex++
            }
            if (count > 0) {
              const r = redact(
                `${t.conversationId}${t.agentId ? ` agent=${t.agentId}` : ''} #${line.index} ${timestampOf(entry) || '-'} ${kind} (${count} match${count === 1 ? '' : 'es'})\n` +
                  snippets.map((s) => `  ${s}`).join('\n') +
                  '\n',
              )
              if (out.length + r.text.length > budget && results > 0) {
                next = { k: t.key, i: line.index, b: line.start, c: 0 }
                break scan
              }
              out += r.text.slice(0, budget)
              redactions += r.count
              results++
              if (results >= maxResults) {
                next = after
                break scan
              }
            }
          }
        }
        if (scanned >= SCAN_BUDGET_BYTES) {
          next = after
          break scan
        }
      }
    } finally {
      await fh.close()
    }
  }
  const footer = JSON.stringify({
    query_kind: isRegex ? 'regex' : 'literal',
    transcripts: list.length,
    results,
    scanned_bytes: scanned,
    next_cursor: next ? encodeCursor(next) : null,
    done: next === null,
    redactions,
  })
  return { blocks: [out || '(no matches)', footer], entries: results }
}
