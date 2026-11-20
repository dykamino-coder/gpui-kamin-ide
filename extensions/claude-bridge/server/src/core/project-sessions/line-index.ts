// Sparse line index of a transcript: the byte offset of every STRIDE-th line.
//
// `from_index` and `tail` used to walk the whole file on every call, so paging
// through a 200 MB conversation read it again for each page. The index is built
// once by a newline scan (no JSON parsing) and then only extended over the bytes
// appended since: transcripts grow at the end. A different file (identity), a
// shorter one, or a same-size rewrite starts it over. Memory is one number per
// STRIDE lines; transcript text is never kept.
//
// The offsets are also kept on disk, in the conversation digest next to the
// log (engine/transcript-digest.ts): when it exists, the index starts from it
// and scans only what follows; when it does not, the scan here serves the
// call (the chat tools never write; the server builds the digest as the log
// grows, engine/transcript-digest.ts).

import fs from 'node:fs'
import type fsp from 'node:fs/promises'
import { DIGEST_STRIDE, digestOf, sidecarOf } from '../engine/transcript-digest'
import { readLines, type Line } from './line-reader'

export const STRIDE = DIGEST_STRIDE
const SCAN_CHUNK = 1024 * 1024
const CACHE_LIMIT = 256

interface LineIndex {
  identity: string
  /** File size and mtime the index was last brought up to date with. */
  size: number
  mtimeMs: number
  /** Byte just past the last newline seen (a line start). */
  scanned: number
  /** Complete lines before `scanned`. */
  lines: number
  /** offsets[k] = first byte of line k * STRIDE. */
  offsets: number[]
}

const cache = new Map<string, LineIndex>()

/** Scans performed and bytes they read — lets tests and the bench prove reuse. */
export const indexStats = { builds: 0, extends: 0, bytesScanned: 0, fromDigest: 0 }

export async function lineIndex(file: string, fh: fsp.FileHandle): Promise<LineIndex> {
  const st = await fh.stat()
  const identity = `${st.dev}:${st.ino}:${st.birthtimeMs}`
  const cached = cache.get(file)
  let base: LineIndex | undefined = cached
  if (
    base &&
    (base.identity !== identity || st.size < base.size || (st.size === base.size && st.mtimeMs !== base.mtimeMs))
  )
    base = undefined
  if (base && base.size === st.size) {
    cache.delete(file)
    cache.set(file, base)
    return base
  }
  if (!base) {
    const dg = fs.existsSync(sidecarOf(file)) ? await digestOf(file, undefined, { persist: false }) : null
    if (dg && dg.identity === identity && dg.end <= st.size) {
      indexStats.fromDigest++
      base = { identity, size: dg.end, mtimeMs: 0, scanned: dg.end, lines: dg.lines, offsets: dg.offsets }
    }
  }
  // Extend a copy: a concurrent caller keeps using the published snapshot.
  const idx: LineIndex = base
    ? { ...base, offsets: base.offsets.slice() }
    : { identity, size: 0, mtimeMs: 0, scanned: 0, lines: 0, offsets: [0] }
  if (base) indexStats.extends++
  else indexStats.builds++
  const buf = Buffer.allocUnsafe(SCAN_CHUNK)
  let pos = idx.scanned
  while (pos < st.size) {
    const { bytesRead } = await fh.read(buf, 0, Math.min(SCAN_CHUNK, st.size - pos), pos)
    if (bytesRead === 0) break
    indexStats.bytesScanned += bytesRead
    for (let nl = buf.indexOf(10, 0); nl !== -1 && nl < bytesRead; nl = buf.indexOf(10, nl + 1)) {
      idx.lines++
      idx.scanned = pos + nl + 1
      if (idx.lines % STRIDE === 0) idx.offsets.push(idx.scanned)
    }
    pos += bytesRead
  }
  idx.size = st.size
  idx.mtimeMs = st.mtimeMs
  cache.delete(file)
  cache.set(file, idx)
  if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value!)
  return idx
}

/** Physical lines, counting a trailing unterminated fragment. */
export function totalLines(idx: LineIndex): number {
  return idx.lines + (idx.scanned < idx.size ? 1 : 0)
}

/** The line with this index, read from the nearest indexed offset (at most
 *  STRIDE lines), or null past the end. */
export async function seekLine(fh: fsp.FileHandle, idx: LineIndex, index: number): Promise<Line | null> {
  if (index < 0 || index >= totalLines(idx)) return null
  const k = Math.floor(index / STRIDE)
  for await (const line of readLines(fh, idx.offsets[k]!, k * STRIDE)) {
    if (line.index === index) return line
    if (line.index > index) break
  }
  return null
}

/** First of the last `n` lines for which `matches` holds, scanning index blocks
 *  backwards from the end and stopping once `n` are found. */
export async function tailStart(
  fh: fsp.FileHandle,
  idx: LineIndex,
  n: number,
  matches: (text: string | null) => boolean,
): Promise<Line | null> {
  let found: Line[] = []
  for (let k = idx.offsets.length - 1; k >= 0 && found.length < n; k--) {
    const end = k + 1 < idx.offsets.length ? idx.offsets[k + 1]! : Number.POSITIVE_INFINITY
    const block: Line[] = []
    for await (const line of readLines(fh, idx.offsets[k]!, k * STRIDE)) {
      if (line.start >= end || line.start >= idx.size) break
      if (matches(line.text)) block.push({ ...line, text: null })
    }
    found = block.concat(found).slice(-n)
  }
  return found[0] ?? null
}

/** Tests: forget every index. */
export function resetLineIndexForTests(): void {
  cache.clear()
  indexStats.builds = 0
  indexStats.extends = 0
  indexStats.bytesScanned = 0
  indexStats.fromDigest = 0
}
