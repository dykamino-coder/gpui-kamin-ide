// project_session_read and project_session_raw: paged, byte-capped reading of
// one transcript inside the caller's project (INC-2026-0072).

import {
  ArgError,
  boolArg,
  decodeCursor,
  encodeCursor,
  kindsArg,
  maxCharsArg,
  optIntArg,
  timeArg,
  type Cursor,
} from './args'
import { openTranscript, readByteRange, readLineContinuation, readLines } from './line-reader'
import { redact } from './redact'
import {
  classify,
  defaultKinds,
  inTimeRange,
  parseEntry,
  renderBody,
  renderHeader,
  type Kind,
  type RenderOptions,
} from './render'
import type { CallerContext } from './scope'
import { resolveTarget, startPosition } from './target'
import type { ToolOutput } from './types'

const MAX_TAIL = 1000
const MAX_INDEX = Number.MAX_SAFE_INTEGER

function pageFooter(fields: Record<string, unknown>): string {
  return JSON.stringify(fields)
}

export async function readConversation(ctx: CallerContext, args: Record<string, unknown>): Promise<ToolOutput> {
  const detail = args.detail === undefined || args.detail === null ? 'text' : args.detail
  if (detail !== 'text' && detail !== 'full') throw new ArgError('detail must be "text" or "full"')
  const opts: RenderOptions = {
    detail,
    includeTools: boolArg(args.include_tools, 'include_tools', detail === 'full'),
    includeThinking: boolArg(args.include_thinking, 'include_thinking', detail === 'full'),
  }
  const kinds: Set<Kind> = kindsArg(args.kinds) ?? defaultKinds(opts)
  const since = timeArg(args.since, 'since')
  const until = timeArg(args.until, 'until')
  const fromIndex = optIntArg(args.from_index, 'from_index', 0, MAX_INDEX)
  const toIndex = optIntArg(args.to_index, 'to_index', 0, MAX_INDEX)
  const tail = optIntArg(args.tail, 'tail', 1, MAX_TAIL)
  const cursor = decodeCursor(args.cursor)
  const maxChars = maxCharsArg(args.max_chars)
  const target = await resolveTarget(ctx, args)

  const matches = (text: string | null): boolean => {
    if (text === null) return true
    const entry = parseEntry(text)
    return !!entry && kinds.has(classify(entry)) && inTimeRange(entry, since, until)
  }
  const fh = await openTranscript(target.file)
  try {
    const start = await startPosition(fh, cursor, fromIndex, tail, matches)
    let out = ''
    let entries = 0
    let firstIndex: number | null = null
    let lastIndex: number | null = null
    let redactions = 0
    let next: Cursor | null = null
    const budget = maxChars - 600
    if (start) {
      for await (const line of readLines(fh, start.byte, start.index)) {
        if (toIndex !== null && line.index >= toIndex) break
        let piece: string
        if (line.text === null) {
          piece = `#${line.index} oversized entry (${line.end - line.start} bytes) — use project_session_raw\n\n`
        } else {
          const entry = parseEntry(line.text)
          if (!entry) continue
          const kind = classify(entry)
          if (!kinds.has(kind) || !inTimeRange(entry, since, until)) continue
          const body = renderBody(entry, kind, opts)
          if (body === '') continue
          const r = redact(`${renderHeader(line.index, entry, kind, opts)}\n${body}\n\n`)
          redactions += r.count
          piece = r.text
        }
        const offset = line.index === start.index ? start.offset : 0
        if (offset > 0) piece = piece.slice(offset)
        if (out.length + piece.length <= budget) {
          out += piece
        } else if (out.length === 0) {
          out = piece.slice(0, budget)
          next = { i: line.index, b: line.start, c: offset + budget }
        } else {
          next = { i: line.index, b: line.start, c: 0 }
          break
        }
        entries++
        firstIndex ??= line.index
        lastIndex = line.index
        if (next) break
      }
    }
    const footer = pageFooter({
      conversation_id: target.ref.conversationId,
      agent_id: target.agentId,
      source: target.ref.source,
      entries,
      first_index: firstIndex,
      last_index: lastIndex,
      partial_entry: next !== null && next.c > 0,
      next_cursor: next ? encodeCursor(next) : null,
      done: next === null,
      redactions,
    })
    return {
      blocks: [out || '(no matching entries)', footer],
      conversationId: target.ref.conversationId,
      self: target.self,
      entries,
    }
  } finally {
    await fh.close()
  }
}

export async function readRaw(ctx: CallerContext, args: Record<string, unknown>): Promise<ToolOutput> {
  const fromIndex = optIntArg(args.from_index, 'from_index', 0, MAX_INDEX)
  const toIndex = optIntArg(args.to_index, 'to_index', 0, MAX_INDEX)
  const tail = optIntArg(args.tail, 'tail', 1, MAX_TAIL)
  const cursor = decodeCursor(args.cursor)
  const maxChars = maxCharsArg(args.max_chars)
  const target = await resolveTarget(ctx, args)
  const fh = await openTranscript(target.file)
  try {
    const start = await startPosition(fh, cursor, fromIndex, tail, null)
    let out = ''
    let lines = 0
    let firstIndex: number | null = null
    let lastIndex: number | null = null
    let redactions = 0
    let next: Cursor | null = null
    const budget = maxChars - 600
    let resume = start
    if (start && cursor?.o === 1) {
      // Resume inside an oversized line by bytes, without rescanning it.
      const cont = await readLineContinuation(fh, start.byte + start.offset, budget)
      const r = redact(cont.bytes.toString('utf8'))
      redactions += r.count
      out = r.text
      firstIndex = lastIndex = start.index
      if (cont.lineEnded) {
        out += '\n'
        lines++
        resume = { index: start.index + 1, byte: start.byte + start.offset + cont.bytes.length + 1, offset: 0 }
      } else {
        next = { i: start.index, b: start.byte, c: start.offset + cont.bytes.length, o: 1 }
        resume = null
      }
    }
    if (resume) {
      for await (const line of readLines(fh, resume.byte, resume.index)) {
        if (toIndex !== null && line.index >= toIndex) break
        const offset = line.index === resume.index ? resume.offset : 0
        firstIndex ??= line.index
        if (line.text !== null) {
          // Offsets inside an ordinary line count characters of its redacted text.
          const r = redact(line.text)
          const rest = r.text.slice(offset) + '\n'
          if (out.length + rest.length <= budget) {
            out += rest
            redactions += r.count
            lines++
            lastIndex = line.index
            continue
          }
          if (out.length > 0) {
            next = { i: line.index, b: line.start, c: 0 }
            break
          }
          out = rest.slice(0, budget)
          redactions += r.count
          lastIndex = line.index
          next = { i: line.index, b: line.start, c: offset + budget }
          break
        }
        // Oversized line: stream it by bytes; offsets count bytes into the line.
        const room = budget - out.length
        if (room < 4096 && out.length > 0) {
          next = { i: line.index, b: line.start, c: 0 }
          break
        }
        const from = line.start + offset
        const chunk = await readByteRange(fh, from, Math.min(line.end, from + room), line.end)
        const r = redact(chunk.toString('utf8'))
        redactions += r.count
        out += r.text
        lastIndex = line.index
        const used = offset + chunk.length
        if (line.start + used < line.end) {
          next = { i: line.index, b: line.start, c: used, o: 1 }
          break
        }
        out += '\n'
        lines++
      }
    }
    const footer = pageFooter({
      conversation_id: target.ref.conversationId,
      agent_id: target.agentId,
      source: target.ref.source,
      lines,
      first_index: firstIndex,
      last_index: lastIndex,
      partial_line: next !== null && next.c > 0,
      next_cursor: next ? encodeCursor(next) : null,
      done: next === null,
      redactions,
    })
    return { blocks: [out, footer], conversationId: target.ref.conversationId, self: target.self, entries: lines }
  } finally {
    await fh.close()
  }
}
