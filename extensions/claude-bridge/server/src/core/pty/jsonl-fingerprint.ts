// Identity of a transcript file, cheap enough to check on every reconnect.
//
// A client that mirrors the transcript locally has to know one thing before it
// trusts its copy: is the file on the server still the SAME file, merely longer?
// The CLI rewrites the transcript when it compacts the conversation, and a
// mirror that is no longer a prefix of the server's file must not be extended —
// that would splice new records onto stale ones and produce a chat that never
// existed.
//
// Size+mtime alone is not enough (a rewrite can land on the same size, and
// mtime granularity is coarse), so the head of the file is hashed too. This is
// the approach Filebeat settled on for the same problem — file identity by a
// hash of the first N bytes rather than inode or size, because rotation and
// rewrites break the cheap signals.
//
// Cost: one 1KB read plus a stat. Not a 85MB re-read.
import { createHash } from 'crypto'
import fsp from 'fs/promises'

/** Bytes of the head that go into the hash. A compaction rewrites from the very
 *  beginning, so the first KB changes; a plain append never touches it. */
const HEAD_BYTES = 1024

export interface TranscriptFingerprint {
  /** sha256 of the first HEAD_BYTES (hex, truncated — collisions here are not a
   *  security boundary, just a change detector). */
  head: string
  /** Total size in bytes; doubles as the "how much do you have" watermark. */
  size: number
}

export async function fingerprintTranscript(filePath: string): Promise<TranscriptFingerprint | null> {
  let fd: fsp.FileHandle | undefined
  try {
    const stat = await fsp.stat(filePath)
    fd = await fsp.open(filePath, 'r')
    const buf = Buffer.alloc(Math.min(HEAD_BYTES, stat.size))
    if (buf.length > 0) await fd.read(buf, 0, buf.length, 0)
    return {
      head: createHash('sha256').update(buf).digest('hex').slice(0, 32),
      size: stat.size,
    }
  } catch {
    return null // missing/unreadable — caller treats it as "cannot resume"
  } finally {
    await fd?.close().catch(() => { /* already gone */ })
  }
}

/** Can a client holding `clientHead` at `clientPos` be topped up rather than
 *  re-sent everything?
 *
 *  Three ways this is false, and each MUST fall back to a full resend:
 *   - the head changed  → the file was rewritten (compaction)
 *   - the file shrank below the client's position → truncated
 *   - no fingerprint at all → nothing to compare against
 *
 *  Being wrong in the permissive direction corrupts the client's history, so
 *  every uncertainty resolves to "resend". */
export function canResume(
  current: TranscriptFingerprint | null,
  clientHead: string | undefined,
  clientPos: number | undefined,
): boolean {
  if (!current || !clientHead || clientPos === undefined) return false
  if (current.head !== clientHead) return false
  return clientPos <= current.size
}

/** Does a record actually START at this offset?
 *
 *  The head hash alone is NOT enough. `repairJsonl` rewrites the transcript
 *  before a resume — dropping torn lines, relinking parents — and a repair that
 *  removes a line from the MIDDLE leaves the first KB untouched while shifting
 *  every offset after it. The head then matches, the file is still longer than
 *  the client's cursor, and the resume is allowed straight into the middle of a
 *  record. Reproduced on a real transcript: the client's offset landed at
 *  `"user","message":{"role":"user","content":[{"tool_use_id":"t` — a fragment
 *  that would have been spliced into its history as if it were a record.
 *
 *  So the boundary is checked directly: read a few bytes at the offset and
 *  require a record to begin there. Cheap (one small read) and it catches every
 *  shift, wherever the edit happened. */
/** Does the record the client says it holds still sit exactly where it says?
 *
 *  This is the PROOF; the `{` check below is only a cheap screen. A repair that
 *  removes one record shifts everything after it by that record's length — and
 *  when records happen to be similar in size, the shifted offset lands on
 *  another record's opening brace. The boundary check passes and the client
 *  silently resumes one record off. Comparing the uuid AT the offset the client
 *  names closes that: same offset, same record, or no resume.
 *
 *  Reads only as far as the record's own line. */
export async function recordUuidMatches(
  filePath: string,
  pos: number | undefined,
  expectedUuid: string | undefined,
): Promise<boolean> {
  // Nothing claimed → nothing to disprove; the caller's other gates decide.
  if (pos === undefined || !expectedUuid) return true
  let fd: fsp.FileHandle | undefined
  try {
    fd = await fsp.open(filePath, 'r')
    const chunks: Buffer[] = []
    let at = pos
    for (let i = 0; i < MAX_RECORD_CHUNKS; i++) {
      const buf = Buffer.alloc(CHUNK)
      const { bytesRead } = await fd.read(buf, 0, CHUNK, at)
      if (bytesRead === 0) break
      const slice = buf.subarray(0, bytesRead)
      const nl = slice.indexOf(0x0a)
      if (nl >= 0) { chunks.push(slice.subarray(0, nl)); break }
      chunks.push(slice)
      at += bytesRead
    }
    const line = Buffer.concat(chunks).toString('utf8')
    if (!line) return false
    const parsed = JSON.parse(line) as { uuid?: string }
    return parsed.uuid === expectedUuid
  } catch {
    return false // unparseable at that offset is itself a mismatch
  } finally {
    await fd?.close().catch(() => { /* already gone */ })
  }
}

const CHUNK = 64 * 1024
/** A single record is bounded in practice; this caps a pathological read. */
const MAX_RECORD_CHUNKS = 64

export async function isRecordBoundary(filePath: string, pos: number): Promise<boolean> {
  if (pos < 0) return false
  let fd: fsp.FileHandle | undefined
  try {
    const stat = await fsp.stat(filePath)
    if (pos === stat.size) return true // exactly the end: nothing new yet
    if (pos > stat.size) return false
    fd = await fsp.open(filePath, 'r')
    // A JSONL record starts with `{`. Reading one byte is enough to tell a
    // boundary from a landing inside a record's text.
    const buf = Buffer.alloc(1)
    const { bytesRead } = await fd.read(buf, 0, 1, pos)
    return bytesRead === 1 && buf[0] === 0x7b // '{'
  } catch {
    return false
  } finally {
    await fd?.close().catch(() => { /* already gone */ })
  }
}
