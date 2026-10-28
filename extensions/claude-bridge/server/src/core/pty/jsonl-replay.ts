// ============================================================================
// Bounded replay pipeline: transcript file → WebSocket.
//
// The replay used to parse the WHOLE transcript into one array, keep it alive
// for the whole (backpressure-stretched) emission, and cut frames by RECORD
// count (200). A 177 MiB transcript therefore lived in memory several times per
// replaying client, and 200 large records made a frame that the 16 MiB send cap
// rejects even on an idle socket — retried every 250 ms forever.
//
// Now a replay is two streaming passes:
//   1. index — read forward once, parse each record once (stats hooks, segment
//      index, compact boundaries) and keep ONLY its byte range (16 bytes per
//      record instead of the record);
//   2. emit — cut batches by SOURCE BYTES (and a record cap), read each batch's
//      byte range back from disk, parse, send, drop. Memory is one batch.
// Delivery waits for the socket to drain instead of re-serialising a rejected
// frame, distinguishes a frame that can never fit from temporary pressure, and
// stops on cancellation.
// ============================================================================

import type { JsonlEntry } from '../../shared/jsonl-types'
import { readRange } from './jsonl-lines'

/** Upper bound of source bytes in one replay frame. Well under the 16 MiB
 *  per-connection cap, so a batch is only refused for real pressure. */
export const REPLAY_BATCH_BYTES = 1024 * 1024
export const REPLAY_BATCH_RECORDS = 200

/** Growable list of record byte ranges (start inclusive, end exclusive). */
export class RecordIndex {
  private starts = new Float64Array(1024)
  private ends = new Float64Array(1024)
  length = 0

  push(start: number, end: number): void {
    if (this.length === this.starts.length) {
      const grow = (a: Float64Array<ArrayBuffer>): Float64Array<ArrayBuffer> => {
        const b = new Float64Array(a.length * 2)
        b.set(a)
        return b
      }
      this.starts = grow(this.starts)
      this.ends = grow(this.ends)
    }
    this.starts[this.length] = start
    this.ends[this.length] = end
    this.length++
  }

  start(i: number): number {
    return this.starts[i]!
  }

  end(i: number): number {
    return this.ends[i]!
  }
}

/** Largest batch `[from, to)` starting at `from` (forward) within the budgets;
 *  always at least one record. */
export function batchForward(idx: RecordIndex, from: number, limit: number): number {
  let to = from + 1
  while (to < limit && to - from < REPLAY_BATCH_RECORDS && idx.end(to) - idx.start(from) <= REPLAY_BATCH_BYTES) {
    to++
  }
  return to
}

/** Largest batch `[from, to)` ending at `to` (backward) within the budgets. */
export function batchBackward(idx: RecordIndex, to: number, limit: number): number {
  let from = to - 1
  while (
    from > limit &&
    to - from < REPLAY_BATCH_RECORDS &&
    idx.end(to - 1) - idx.start(from - 1) <= REPLAY_BATCH_BYTES
  ) {
    from--
  }
  return from
}

export type Tagged = JsonlEntry & { _pos?: number; _posEnd?: number; _ord?: number }

/** Read records `[from, to)` back from disk and tag them. A record that no
 *  longer parses (file changed under us) is skipped; the caller's identity
 *  check turns a real rewrite into a fresh replay. */
export async function loadBatch(
  filePath: string,
  idx: RecordIndex,
  from: number,
  to: number,
  ordBase: number,
): Promise<Tagged[]> {
  const rangeStart = idx.start(from)
  const buf = await readRange(filePath, rangeStart, idx.end(to - 1))
  const out: Tagged[] = []
  for (let k = from; k < to; k++) {
    const a = idx.start(k) - rangeStart
    let b = idx.end(k) - rangeStart
    if (b > buf.length) break
    while (b > a && (buf[b - 1] === 0x0a || buf[b - 1] === 0x0d)) b--
    let parsed: Tagged
    try {
      parsed = JSON.parse(buf.toString('utf8', a, b)) as Tagged
    } catch {
      continue
    }
    parsed._pos = idx.start(k)
    parsed._posEnd = idx.end(k)
    parsed._ord = ordBase + k
    out.push(parsed)
  }
  return out
}

/** Bytes of the source span a batch covers (for the progress bar). */
export function batchSpan(idx: RecordIndex, from: number, to: number): number {
  let span = 0
  for (let k = from; k < to; k++) span += idx.end(k) - idx.start(k)
  return span
}

/** Result of one send attempt, as reported by the transport. A plain boolean
 *  (legacy callbacks) maps to sent / backpressured. */
export type SendOutcome = 'sent' | 'backpressured' | 'oversized' | 'closed'

export function normalizeOutcome(r: boolean | SendOutcome | void): SendOutcome {
  if (r === true || r === undefined) return 'sent'
  if (r === false) return 'backpressured'
  return r
}

/** A stand-in for one record that can never fit a frame. It keeps the chain
 *  fields (uuid/parentUuid/type/timestamp) and the cursor tags so the client's
 *  ordering, dedup and mirror cursor stay correct, and says why the content is
 *  missing instead of silently dropping it or retrying forever. */
export function oversizedStub(e: Tagged, bytes: number): Tagged {
  const mib = (bytes / (1024 * 1024)).toFixed(1)
  const msg = e.message as { role?: string; id?: string; model?: string } | undefined
  return {
    type: e.type,
    uuid: e.uuid,
    parentUuid: e.parentUuid,
    timestamp: e.timestamp,
    isSidechain: e.isSidechain,
    ...(msg
      ? {
          message: {
            role: msg.role,
            id: msg.id,
            model: msg.model,
            content: [{ type: 'text', text: `[Bridge: this record is ${mib} MiB — too large to deliver to the chat]` }],
          },
        }
      : {}),
    _pos: e._pos,
    _posEnd: e._posEnd,
    _ord: e._ord,
    _oversized: bytes,
  } as unknown as Tagged
}
