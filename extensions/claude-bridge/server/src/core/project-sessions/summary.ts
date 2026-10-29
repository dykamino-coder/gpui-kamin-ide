// Request-driven metadata only. Cache summaries, never transcript text. Line
// counts advance by a bounded byte range and stay null until the cursor catches
// up; a list of hundreds of agents must not read hundreds of whole transcripts.
import type { Stats } from 'node:fs'
import { headTail, openTranscript } from './line-reader'
import { classify, parseEntry, promptText, timestampOf, type Entry } from './render'

export const SUMMARY_BYTES = 128 * 1024
export const COUNT_SLICE_BYTES = 256 * 1024
const CACHE_LIMIT = 512

interface Summary {
  title: string | null
  startedAt: string | null
  lastEntryAt: string | null
  lines: number | null
}
interface Cached {
  identity: string
  size: number
  mtimeMs: number
  offset: number
  newlines: number
  lastByte: number
  summary: Summary
}
const cache = new Map<string, Cached>()

export async function summarize(file: string, stat: Stats): Promise<Summary> {
  const identity = `${stat.dev}:${stat.ino}:${stat.birthtimeMs}`
  let c = cache.get(file)
  if (c && (c.identity !== identity || stat.size < c.size || (stat.size === c.size && stat.mtimeMs !== c.mtimeMs)))
    c = undefined
  if (c && c.size === stat.size && c.mtimeMs === stat.mtimeMs && c.offset >= stat.size) {
    cache.delete(file)
    cache.set(file, c)
    return c.summary
  }
  // Never mutate the cached cursor across an await. Concurrent list requests
  // may read the same slice; publishing immutable snapshots keeps it counted once.
  if (c) c = { ...c, summary: { ...c.summary } }
  const fh = await openTranscript(file)
  try {
    if (!c || c.size !== stat.size || c.mtimeMs !== stat.mtimeMs) {
      const { head, tail } = await headTail(fh, SUMMARY_BYTES)
      const heads = head.map(parseEntry).filter((e): e is Entry => e !== null)
      const tails = tail.map(parseEntry).filter((e): e is Entry => e !== null)
      let custom: string | null = null
      for (const e of [...heads, ...tails]) {
        if (e.type === 'custom-title' && typeof e.customTitle === 'string') custom = e.customTitle
      }
      const first = heads.find((e) => classify(e) === 'user' && promptText(e).trim() !== '')
      c = {
        identity,
        size: stat.size,
        mtimeMs: stat.mtimeMs,
        offset: c?.offset ?? 0,
        newlines: c?.newlines ?? 0,
        lastByte: c?.lastByte ?? 10,
        summary: {
          title: (custom ?? (first ? promptText(first) : null))?.slice(0, 512) ?? null,
          startedAt: heads.map(timestampOf).find(Boolean) ?? null,
          lastEntryAt: [...tails].reverse().map(timestampOf).find(Boolean) ?? null,
          lines: null,
        },
      }
    }
    const b = Buffer.allocUnsafe(Math.min(COUNT_SLICE_BYTES, stat.size - c.offset))
    if (b.length) {
      const { bytesRead } = await fh.read(b, 0, b.length, c.offset)
      for (let i = 0; i < bytesRead; i++) if (b[i] === 10) c.newlines++
      if (bytesRead) c.lastByte = b[bytesRead - 1]!
      c.offset += bytesRead
    }
    c.summary.lines = c.offset >= stat.size ? c.newlines + (c.lastByte === 10 ? 0 : 1) : null
    cache.delete(file)
    cache.set(file, c)
    if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value!)
    return c.summary
  } finally {
    await fh.close()
  }
}
