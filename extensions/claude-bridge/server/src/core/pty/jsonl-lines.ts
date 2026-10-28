// ============================================================================
// Byte-oriented, cancellable JSONL line reader shared by every path that walks
// a transcript on the request/resume path (replay, sync, segment, repair).
//
// Why not `buf.toString()` per chunk + `split('\n')`: decoding each chunk on
// its own splits a multi-byte UTF-8 character that straddles the chunk edge
// into replacement characters, and byte offsets derived from the decoded text
// then drift (+N bytes per broken character). LF (0x0A) never occurs inside a
// UTF-8 multi-byte sequence, so lines are framed on raw bytes and each COMPLETE
// line is decoded once — offsets are exact and characters survive.
//
// Memory is one read chunk plus the line currently being assembled; nothing is
// accumulated across lines. The caller bounds its own batches.
//
// Cooperative scheduling: the reader yields to the event loop (`setImmediate`)
// after every `yieldEveryBytes` of consumed input, so a caller that parses each
// line stays within a few milliseconds per slice and HTTP/WS keep being served.
// ============================================================================

import fsp from 'fs/promises'

export interface JsonlLine {
  /** Decoded line text without the trailing LF (and without a trailing CR). */
  text: string
  /** Byte offset where the line starts. */
  start: number
  /** Byte offset just past the line's LF (or EOF/`end` for an unterminated line). */
  end: number
  /** False for the final line when the range ended without an LF. */
  terminated: boolean
}

export interface ReadJsonlLinesOptions {
  /** First byte to read (must be a line boundary for exact offsets). */
  start?: number
  /** Stop before this byte (default: the size seen at open time). */
  end?: number
  signal?: AbortSignal
  chunkBytes?: number
  /** Yield to the event loop after this many consumed bytes. */
  yieldEveryBytes?: number
}

const DEFAULT_CHUNK = 1024 * 1024
const DEFAULT_YIELD = 256 * 1024

export function yieldToEventLoop(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve))
}

/** Async-iterate complete JSONL lines of `filePath` in the byte range
 *  `[start, end)`. Empty lines are skipped. Throws `AbortError` once `signal`
 *  aborts: no further read, decode or callback happens after that point. */
export async function* readJsonlLines(filePath: string, opts: ReadJsonlLinesOptions = {}): AsyncGenerator<JsonlLine> {
  const chunkBytes = opts.chunkBytes ?? DEFAULT_CHUNK
  const yieldEvery = opts.yieldEveryBytes ?? DEFAULT_YIELD
  const signal = opts.signal
  signal?.throwIfAborted()
  const fd = await fsp.open(filePath, 'r')
  try {
    const end = opts.end ?? (await fd.stat()).size
    let pos = opts.start ?? 0
    // Bytes of the line currently being assembled (from a previous chunk).
    let carry: Buffer[] = []
    let carryLen = 0
    let lineStart = pos
    let sinceYield = 0
    const buf = Buffer.allocUnsafe(chunkBytes)
    while (pos < end) {
      signal?.throwIfAborted()
      const want = Math.min(chunkBytes, end - pos)
      const { bytesRead } = await fd.read(buf, 0, want, pos)
      signal?.throwIfAborted()
      if (bytesRead === 0) break
      let from = 0
      while (from < bytesRead) {
        const nl = buf.indexOf(0x0a, from)
        if (nl === -1 || nl >= bytesRead) {
          // Copy: `buf` is reused by the next read.
          const part = Buffer.from(buf.subarray(from, bytesRead))
          carry.push(part)
          carryLen += part.length
          break
        }
        let text: string
        if (carryLen > 0) {
          carry.push(buf.subarray(from, nl))
          text = decodeLine(Buffer.concat(carry, carryLen + (nl - from)))
          carry = []
          carryLen = 0
        } else {
          text = decodeLine(buf.subarray(from, nl))
        }
        const lineEnd = pos + nl + 1
        const ls = lineStart
        lineStart = lineEnd
        from = nl + 1
        if (text.length > 0 && text.trim().length > 0) {
          yield { text, start: ls, end: lineEnd, terminated: true }
          signal?.throwIfAborted()
        }
      }
      pos += bytesRead
      sinceYield += bytesRead
      if (sinceYield >= yieldEvery) {
        sinceYield = 0
        await yieldToEventLoop()
      }
    }
    if (carryLen > 0) {
      signal?.throwIfAborted()
      const text = decodeLine(Buffer.concat(carry, carryLen))
      if (text.trim().length > 0) yield { text, start: lineStart, end: lineStart + carryLen, terminated: false }
    }
  } finally {
    await fd.close()
  }
}

function decodeLine(bytes: Buffer): string {
  // Tolerate CRLF writers: the CR belongs to the line terminator, not the JSON.
  const n = bytes.length > 0 && bytes[bytes.length - 1] === 0x0d ? bytes.length - 1 : bytes.length
  return bytes.toString('utf8', 0, n)
}

/** Read `[start, end)` of a file into one Buffer (bounded by the caller). */
export async function readRange(filePath: string, start: number, end: number): Promise<Buffer> {
  const len = Math.max(0, end - start)
  const out = Buffer.allocUnsafe(len)
  const fd = await fsp.open(filePath, 'r')
  try {
    let got = 0
    while (got < len) {
      const { bytesRead } = await fd.read(out, got, len - got, start + got)
      if (bytesRead === 0) break
      got += bytesRead
    }
    return got === len ? out : out.subarray(0, got)
  } finally {
    await fd.close()
  }
}
