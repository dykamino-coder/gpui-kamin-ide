// Resolve a tool's (conversation_id, agent_id) arguments to one transcript file
// inside the caller's scope, plus the shared start-position logic of paged tools.

import type fsp from 'node:fs/promises'
import { ArgError, type Cursor } from './args'
import { isLineStart, readLines } from './line-reader'
import { locateSubagent, locateTranscript, resolveScope, type CallerContext, type TranscriptRef } from './scope'

export interface Target {
  ref: TranscriptRef
  file: string
  agentId: string | null
  self: boolean
}

export async function resolveTarget(ctx: CallerContext, args: Record<string, unknown>): Promise<Target> {
  const scope = resolveScope(ctx)
  const ref = await locateTranscript(ctx, scope, args.conversation_id)
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
 *  lines count toward `tail` (null = every line). */
export async function startPosition(
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
    const ring: Array<{ index: number; start: number }> = []
    for await (const line of readLines(fh)) {
      if (matches && !matches(line.text)) continue
      ring.push({ index: line.index, start: line.start })
      if (ring.length > tail) ring.shift()
    }
    const first = ring[0]
    return first ? { index: first.index, byte: first.start, offset: 0 } : null
  }
  if (fromIndex !== null && fromIndex > 0) {
    for await (const line of readLines(fh)) {
      if (line.index === fromIndex) return { index: line.index, byte: line.start, offset: 0 }
    }
    return null
  }
  return { index: 0, byte: 0, offset: 0 }
}
