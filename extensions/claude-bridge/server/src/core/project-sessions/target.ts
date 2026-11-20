// Resolve a tool's (session_id, agent_id) arguments to one transcript file
// inside the caller's scope, plus the shared start-position logic of paged tools.

import type fsp from 'node:fs/promises'
import { ArgError, type Cursor } from './args'
import { isLineStart } from './line-reader'
import { lineIndex, seekLine, tailStart, totalLines } from './line-index'
import { locateSubagent, locateTranscript, resolveScope, type CallerContext, type TranscriptRef } from './scope'

export interface Target {
  ref: TranscriptRef
  file: string
  agentId: string | null
  self: boolean
}

export async function resolveTarget(ctx: CallerContext, args: Record<string, unknown>): Promise<Target> {
  const scope = resolveScope(ctx)
  const ref = await locateTranscript(ctx, scope, args.session_id)
  const self = ctx.selfIds.includes(ref.conversationId)
  if (args.agent_id === undefined || args.agent_id === null || args.agent_id === '') {
    return { ref, file: ref.file, agentId: null, self }
  }
  const file = await locateSubagent(ctx, ref, args.agent_id)
  return { ref, file, agentId: String(args.agent_id).replace(/^agent-/, ''), self }
}

export interface StartPos {
  index: number
  byte: number
  offset: number
}

/** Where a paged read starts: an explicit cursor, the first of the last `tail`
 *  matching entries, `from_index`, or the beginning. `matches` decides which
 *  lines count toward `tail` (null = every line). `tail` and `from_index` seek
 *  through the file's sparse line index instead of reading it from the top. */
export async function startPosition(
  file: string,
  fh: fsp.FileHandle,
  cursor: Cursor | null,
  fromIndex: number | null,
  tail: number | null,
  matches: ((text: string | null) => boolean) | null,
): Promise<StartPos | null> {
  if (cursor) {
    if (!(await isLineStart(fh, cursor.b))) throw new ArgError('cursor does not match this transcript')
    return { index: cursor.i, byte: cursor.b, offset: cursor.c }
  }
  if (tail !== null) {
    const idx = await lineIndex(file, fh)
    const first = matches
      ? await tailStart(fh, idx, tail, matches)
      : await seekLine(fh, idx, Math.max(0, totalLines(idx) - tail))
    return first ? { index: first.index, byte: first.start, offset: 0 } : null
  }
  if (fromIndex !== null && fromIndex > 0) {
    const line = await seekLine(fh, await lineIndex(file, fh), fromIndex)
    return line ? { index: line.index, byte: line.start, offset: 0 } : null
  }
  return { index: 0, byte: 0, offset: 0 }
}
