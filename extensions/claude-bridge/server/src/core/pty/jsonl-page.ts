import fsp from 'fs/promises'
import type { JsonlEntry } from '../../shared/jsonl-types'
import type { Tagged } from './jsonl-replay'

export const HISTORY_PAGE_BYTES = 512 * 1024
export const HISTORY_PAGE_RECORDS = 200

/** Read backwards by authoritative byte position. A page never parses the
 * prefix of a transcript. `nextPos` also advances over malformed/bookkeeping
 * windows, so an empty page is not mistaken for the start of history. */
export async function readHistoryPage(
  file: string,
  beforePos: number,
  lower = 0,
  count = HISTORY_PAGE_RECORDS,
): Promise<{ records: JsonlEntry[]; nextPos: number; bytesRead: number; endPos: number }> {
  const fh = await fsp.open(file, 'r')
  try {
    const size = (await fh.stat()).size
    lower = Math.min(size, Math.max(0, lower))
    const end = Math.min(size, Math.max(lower, Number.isNaN(beforePos) ? size : beforePos))
    let start = Math.max(lower, end - HISTORY_PAGE_BYTES)
    let buf = Buffer.alloc(end - start)
    let { bytesRead } = await fh.read(buf, 0, buf.length, start)
    // The byte budget is soft for a single oversized record. Extend only when
    // there is no complete line, otherwise the preceding page owns that row.
    while (start > lower && buf.indexOf(10) === buf.lastIndexOf(10)) {
      const previous = Math.max(lower, start - HISTORY_PAGE_BYTES)
      const prefix = Buffer.alloc(start - previous)
      const read = await fh.read(prefix, 0, prefix.length, previous)
      bytesRead += read.bytesRead
      buf = Buffer.concat([prefix.subarray(0, read.bytesRead), buf])
      start = previous
    }
    const endPos = start + buf.lastIndexOf(10) + 1
    const out: JsonlEntry[] = []
    let at = start === lower ? 0 : buf.indexOf(10) + 1
    // No complete line: continue backwards rather than claiming EOF.
    if (at === 0 && start !== lower) return { records: [], nextPos: start, bytesRead, endPos }
    for (; at < bytesRead;) {
      const nl = buf.indexOf(10, at)
      if (nl < 0) break // a live writer's unfinished final record
      const pos = start + at
      const text = buf.toString('utf8', at, nl)
      at = nl + 1
      try {
        const row = JSON.parse(text) as Tagged
        if (!row || typeof row !== 'object') continue
        row._pos = pos
        row._posEnd = start + at
        row._ord = pos
        out.push(row)
      } catch {
        /* skip malformed rows, with forward progress */
      }
    }
    const records = out.slice(-Math.min(HISTORY_PAGE_RECORDS, Math.max(1, count)))
    return { records, nextPos: (records[0] as Tagged | undefined)?._pos ?? start, bytesRead, endPos }
  } finally {
    await fh.close()
  }
}
