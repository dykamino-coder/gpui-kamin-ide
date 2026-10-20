// Bounded streaming access to JSONL transcripts (INC-2026-0072).
//
// Transcripts reach hundreds of megabytes and single lines (a big tool result)
// several megabytes, so nothing here reads a whole file: lines are produced from
// fixed-size positional reads, and a line longer than MAX_LINE_BYTES is reported
// by its byte range without being materialised. The file is opened read-only and,
// where the platform supports it, with O_NOFOLLOW so a symlink swapped in after
// the scope check is refused at open time.

import fs from 'node:fs'
import fsp from 'node:fs/promises'

export const CHUNK_BYTES = 64 * 1024
export const MAX_LINE_BYTES = 16 * 1024 * 1024

export interface Line {
  /** Physical 0-based line number — the entry index every tool uses. */
  index: number
  /** Byte offset of the first byte of the line. */
  start: number
  /** Byte offset just past the line (excluding the newline). */
  end: number
  /** Decoded text, or null when the line exceeds MAX_LINE_BYTES. */
  text: string | null
}

/** Peak bytes held by any reader in this process — lets tests prove the bound. */
export const readerStats = { peakBufferedBytes: 0 }

const OPEN_FLAGS = fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0)

export async function openTranscript(file: string): Promise<fsp.FileHandle> {
  return fsp.open(file, OPEN_FLAGS)
}

/**
 * Yield lines starting at byte `start` (which must be a line start) with index
 * `startIndex`. Stops at EOF or when `shouldStop` returns true for a line.
 * The trailing unterminated fragment is yielded too (a torn write is still text).
 */
export async function* readLines(
  fh: fsp.FileHandle,
  start = 0,
  startIndex = 0,
  maxLineBytes = MAX_LINE_BYTES,
): AsyncGenerator<Line> {
  const buf = Buffer.allocUnsafe(CHUNK_BYTES)
  let pos = start
  let index = startIndex
  let lineStart = start
  let parts: Buffer[] = []
  let partBytes = 0
  let oversized = false
  for (;;) {
    const { bytesRead } = await fh.read(buf, 0, CHUNK_BYTES, pos)
    if (bytesRead === 0) break
    let from = 0
    for (;;) {
      const nl = buf.indexOf(10, from)
      const stop = nl === -1 || nl >= bytesRead ? bytesRead : nl
      const piece = stop - from
      if (!oversized) {
        if (partBytes + piece > maxLineBytes) {
          oversized = true
          parts = []
          partBytes = 0
        } else if (piece > 0) {
          parts.push(Buffer.from(buf.subarray(from, stop)))
          partBytes += piece
          if (partBytes > readerStats.peakBufferedBytes) readerStats.peakBufferedBytes = partBytes
        }
      }
      if (stop === bytesRead) break
      const end = pos + nl
      yield { index, start: lineStart, end, text: oversized ? null : Buffer.concat(parts, partBytes).toString('utf8') }
      index++
      lineStart = end + 1
      parts = []
      partBytes = 0
      oversized = false
      from = nl + 1
    }
    pos += bytesRead
  }
  if (lineStart < pos) {
    yield {
      index,
      start: lineStart,
      end: pos,
      text: oversized ? null : Buffer.concat(parts, partBytes).toString('utf8'),
    }
  }
}

/** True when `offset` is 0 or directly follows a newline — a valid cursor. */
export async function isLineStart(fh: fsp.FileHandle, offset: number): Promise<boolean> {
  if (!Number.isSafeInteger(offset) || offset < 0) return false
  if (offset === 0) return true
  const size = (await fh.stat()).size
  if (offset > size) return false
  const b = Buffer.alloc(1)
  const { bytesRead } = await fh.read(b, 0, 1, offset - 1)
  return bytesRead === 1 && b[0] === 10
}

/** Read bytes [from, to) of a file, shrinking `to` back to a UTF-8 boundary so
 *  a chunked huge line never splits a character. Returns the bytes actually used. */
export async function readByteRange(fh: fsp.FileHandle, from: number, to: number, lineEnd: number): Promise<Buffer> {
  const len = Math.max(0, to - from)
  const b = Buffer.alloc(len)
  const { bytesRead } = await fh.read(b, 0, len, from)
  let n = bytesRead
  if (from + n < lineEnd) {
    // Step back over continuation bytes (10xxxxxx) and the lead byte they follow.
    let k = n
    while (k > 0 && (b[k - 1]! & 0xc0) === 0x80) k--
    if (k > 0 && b[k - 1]! >= 0xc0) k--
    if (k > 0) n = k
  }
  return b.subarray(0, n)
}

/** Continue inside a long line at byte `from`: up to `max` bytes, stopping at the
 *  line's newline when it falls inside the window. Without a newline the slice is
 *  trimmed back to a UTF-8 boundary (unless the file ends there). This lets raw
 *  paging resume mid-line without rescanning the line from its start. */
export async function readLineContinuation(
  fh: fsp.FileHandle,
  from: number,
  max: number,
): Promise<{ bytes: Buffer; lineEnded: boolean }> {
  const b = Buffer.alloc(max)
  const { bytesRead } = await fh.read(b, 0, max, from)
  const nl = b.subarray(0, bytesRead).indexOf(10)
  if (nl !== -1) return { bytes: b.subarray(0, nl), lineEnded: true }
  if (bytesRead < max) return { bytes: b.subarray(0, bytesRead), lineEnded: true }
  return { bytes: await readByteRange(fh, from, from + max, Number.MAX_SAFE_INTEGER), lineEnded: false }
}

/** Count physical lines (including a trailing unterminated fragment). */
export async function countLines(fh: fsp.FileHandle): Promise<number> {
  const buf = Buffer.allocUnsafe(CHUNK_BYTES)
  let pos = 0
  let count = 0
  let last = 10
  for (;;) {
    const { bytesRead } = await fh.read(buf, 0, CHUNK_BYTES, pos)
    if (bytesRead === 0) break
    let i = buf.indexOf(10, 0)
    while (i !== -1 && i < bytesRead) {
      count++
      i = buf.indexOf(10, i + 1)
    }
    last = buf[bytesRead - 1]!
    pos += bytesRead
  }
  return last === 10 ? count : count + 1
}

/** First and last `bytes` of a file as text (partial lines dropped at the cut). */
export async function headTail(fh: fsp.FileHandle, bytes: number): Promise<{ head: string[]; tail: string[] }> {
  const size = (await fh.stat()).size
  const headLen = Math.min(size, bytes)
  const h = Buffer.alloc(headLen)
  await fh.read(h, 0, headLen, 0)
  const headLines = h.toString('utf8').split('\n')
  if (headLen < size) headLines.pop()
  if (size <= bytes) return { head: headLines, tail: headLines }
  const tailLen = Math.min(size, bytes)
  const t = Buffer.alloc(tailLen)
  await fh.read(t, 0, tailLen, size - tailLen)
  const tailLines = t.toString('utf8').split('\n')
  tailLines.shift()
  return { head: headLines, tail: tailLines }
}
