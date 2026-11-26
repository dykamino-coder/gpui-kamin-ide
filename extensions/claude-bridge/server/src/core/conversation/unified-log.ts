// The single normalized transcript of a conversation that spans engines
// (manifest.ts). It is an ordinary Bridge transcript file — one JsonlEntry per
// line — so the client's replay, paging and local mirror work on it unchanged.
//
// Entries keep their uuids. Each segment's native transcript starts its own
// parent chain, so the first entry of a segment (and any entry whose parent the
// log does not hold) is re-parented onto the log's last entry: the client walks
// one chain from the newest entry back, and a segment left on its own chain
// would read as an abandoned branch and vanish from the chat.

import fs from 'node:fs'
import path from 'node:path'
import { randomUUID } from 'node:crypto'
import type { JsonlEntry } from '../../shared/jsonl-types'
import type { AgentEngineId } from '../../shared/agent-events'
import { noteAppended } from '../engine/transcript-digest'

type Entry = JsonlEntry & { uuid?: string; parentUuid?: string | null; subtype?: string }

export const ENGINE_SWITCH_SUBTYPE = 'engine_switch'

/** Rows that hang on the chain but never parent the next entry: sub-agent side
 *  chains and usage records (codex `system/api_usage`). */
function offChain(e: { isSidechain?: unknown; type?: unknown; subtype?: unknown }): boolean {
  return (
    Boolean(e.isSidechain) || (e.type === 'system' && (e.subtype === 'api_usage' || e.subtype === 'codex_turn_request'))
  )
}

/** Bytes read per step by the whole-log scans below: they run on the server's
 *  event loop, so neither holds the log in memory as one string. */
const SCAN_CHUNK = 1024 * 1024

/** Lines of `file` from the end backwards, read in chunks; stops when `visit`
 *  returns true. */
function visitLinesBackwards(file: string, visit: (line: string) => boolean): void {
  let fd: number | undefined
  try {
    fd = fs.openSync(file, 'r')
    let pos = fs.fstatSync(fd).size
    let carry = Buffer.alloc(0)
    const buf = Buffer.allocUnsafe(SCAN_CHUNK)
    while (pos > 0) {
      const len = Math.min(SCAN_CHUNK, pos)
      pos -= len
      fs.readSync(fd, buf, 0, len, pos)
      const data = carry.length ? Buffer.concat([buf.subarray(0, len), carry]) : buf.subarray(0, len)
      let end = data.length
      for (let nl = data.lastIndexOf(10, end - 1); nl !== -1; nl = data.lastIndexOf(10, end - 1)) {
        if (visit(data.subarray(nl + 1, end).toString('utf8'))) return
        end = nl
        if (end === 0) break
      }
      carry = Buffer.from(data.subarray(0, end))
    }
    if (carry.length) visit(carry.toString('utf8'))
  } catch {
    /* no log */
  } finally {
    if (fd !== undefined) fs.closeSync(fd)
  }
}

/** Last main-chain uuid in a transcript file (side chains and usage rows
 *  skipped), read from the end. */
export function lastUuidIn(file: string): string | null {
  let found: string | null = null
  visitLinesBackwards(file, (line) => {
    if (!line.trim()) return false
    try {
      const e = JSON.parse(line) as { uuid?: unknown; isSidechain?: unknown; type?: unknown; subtype?: unknown }
      if (typeof e.uuid === 'string' && e.uuid && !offChain(e)) {
        found = e.uuid
        return true
      }
    } catch {
      /* torn line */
    }
    return false
  })
  return found
}
/** A crash mid-append leaves a line without its newline: cut it, so the next
 *  append does not glue onto it. The copy position (manifest) is saved after
 *  the append, so the pump copies that entry again. */
function cutTornTail(file: string): void {
  let fd: number | undefined
  try {
    fd = fs.openSync(file, 'r+')
    const size = fs.fstatSync(fd).size
    if (size === 0) return
    const tail = Buffer.alloc(Math.min(size, 64 * 1024))
    fs.readSync(fd, tail, 0, tail.length, size - tail.length)
    if (tail[tail.length - 1] === 0x0a) return
    const nl = tail.lastIndexOf(0x0a)
    let keep = nl >= 0 ? size - tail.length + nl + 1 : 0
    if (nl < 0 && size > tail.length) {
      // A torn line longer than the window: find its start the slow way.
      keep = fs.readFileSync(file).lastIndexOf(0x0a) + 1
    }
    fs.ftruncateSync(fd, keep)
  } catch {
    /* no log yet */
  } finally {
    if (fd !== undefined) fs.closeSync(fd)
  }
}

/** Uuids the log holds: an entry copied again (a pump position saved late
 *  before a crash, a compaction successor that repeats its history) is not
 *  appended twice. */
function uuidsIn(file: string): Set<string> {
  const out = new Set<string>()
  let fd: number | undefined
  try {
    fd = fs.openSync(file, 'r')
    const buf = Buffer.allocUnsafe(SCAN_CHUNK)
    let carry = ''
    let pos = 0
    for (;;) {
      const n = fs.readSync(fd, buf, 0, SCAN_CHUNK, pos)
      if (n === 0) break
      pos += n
      // Whole lines only: a uuid never spans two chunks.
      const nl = buf.lastIndexOf(10, n - 1)
      const text = carry + buf.toString('utf8', 0, nl === -1 ? 0 : nl + 1)
      carry = nl === -1 ? carry + buf.toString('utf8', 0, n) : buf.toString('utf8', nl + 1, n)
      for (const m of text.matchAll(/"uuid":"([^"]+)"/g)) out.add(m[1]!)
    }
    for (const m of carry.matchAll(/"uuid":"([^"]+)"/g)) out.add(m[1]!)
  } catch {
    /* no log yet */
  } finally {
    if (fd !== undefined) fs.closeSync(fd)
  }
  return out
}

// One writer per log file: the leaving segment's last copy can still run while
// the joining segment's copy starts (session-core stops the leaving engine in
// parallel with the start), so both must append through the same chain state.
const writers = new Map<string, UnifiedLog>()

export class UnifiedLog {
  private last: string | null
  /** Uuids of the segment being copied: parents inside it are kept. */
  private segment = new Set<string>()
  private readonly seen: Set<string>

  constructor(readonly file: string) {
    cutTornTail(file)
    this.last = lastUuidIn(file)
    this.seen = uuidsIn(file)
  }
  /** The shared writer of `file` (see `writers`). */
  static of(file: string): UnifiedLog {
    const key = path.resolve(file)
    let log = writers.get(key)
    if (!log) writers.set(key, (log = new UnifiedLog(file)))
    return log
  }

  /** Tests: forget shared writers. */
  static resetForTests(): void {
    writers.clear()
  }

  /** Byte length of the log: where the next entry starts. */
  size(): number {
    try {
      return fs.statSync(this.file).size
    } catch {
      return 0
    }
  }

  /** A new segment starts: its entries chain onto what the log holds now. */
  beginSegment(): void {
    this.segment = new Set()
  }

  append(entries: readonly JsonlEntry[]): void {
    if (entries.length === 0) return
    const lines: string[] = []
    for (const raw of entries) {
      const e = { ...(raw as Entry) }
      if (typeof e.uuid === 'string' && e.uuid) {
        if (this.seen.has(e.uuid)) continue
        this.seen.add(e.uuid)
        const keepsParent =
          e.subtype === 'compact_boundary' || (typeof e.parentUuid === 'string' && this.segment.has(e.parentUuid))
        if (!keepsParent && !e.isSidechain) e.parentUuid = this.last
        this.segment.add(e.uuid)
        if (!offChain(e)) this.last = e.uuid
      }
      lines.push(JSON.stringify(e))
    }
    if (lines.length === 0) return
    fs.mkdirSync(path.dirname(this.file), { recursive: true })
    fs.appendFileSync(this.file, lines.join('\n') + '\n')
    noteAppended(this.file)
  }

  /** "→ GPT-6.1-Sol": where the conversation moved to another engine. */
  appendSwitchMarker(
    to: { engine: AgentEngineId; model: string | null; label: string },
    sessionId: string,
  ): JsonlEntry {
    const marker = {
      type: 'system',
      subtype: ENGINE_SWITCH_SUBTYPE,
      uuid: randomUUID(),
      parentUuid: this.last,
      timestamp: new Date().toISOString(),
      sessionId,
      engine: to.engine,
      model: to.model,
      level: 'info',
      content: `→ ${to.label}`,
    } as unknown as JsonlEntry
    this.beginSegment()
    this.append([marker])
    return marker
  }
}

export interface LogItem {
  entry: JsonlEntry
  /** Byte offset of the entry's line; `end` = where the next line starts. */
  offset: number
  end: number
}

/** Entries of a log from byte `from` on, each with its own byte range. */
export function readLogEntries(file: string, from = 0): LogItem[] {
  let buf: Buffer
  try {
    buf = fs.readFileSync(file)
  } catch {
    return []
  }
  const out: LogItem[] = []
  let pos = Math.max(0, from)
  while (pos < buf.length) {
    const nl = buf.indexOf(0x0a, pos)
    if (nl < 0) break // torn last line
    const line = buf.subarray(pos, nl).toString('utf8')
    if (line.trim()) {
      try {
        out.push({ entry: JSON.parse(line) as JsonlEntry, offset: pos, end: nl + 1 })
      } catch {
        /* torn line */
      }
    }
    pos = nl + 1
  }
  return out
}

/** First segment of a conversation that becomes multi-engine: the log starts
 *  as a byte copy of the chat's transcript so far. */
export function seedUnifiedLog(file: string, source: string): void {
  fs.mkdirSync(path.dirname(file), { recursive: true })
  try {
    // A file copy, not a read into memory: the chat so far can be hundreds of MB.
    fs.copyFileSync(source, file)
  } catch {
    /* an empty chat has no transcript yet */
    fs.writeFileSync(file, '', { mode: 0o600 })
    return
  }
  fs.chmodSync(file, 0o600)
  // A torn last line (process killed mid-write) is dropped, not copied.
  let keep = 0
  const fd = fs.openSync(file, 'r+')
  try {
    const size = fs.fstatSync(fd).size
    const buf = Buffer.allocUnsafe(SCAN_CHUNK)
    for (let pos = size; pos > 0 && keep === 0;) {
      const len = Math.min(SCAN_CHUNK, pos)
      pos -= len
      fs.readSync(fd, buf, 0, len, pos)
      const nl = buf.subarray(0, len).lastIndexOf(10)
      if (nl !== -1) keep = pos + nl + 1
    }
    if (keep < size) fs.ftruncateSync(fd, keep)
  } finally {
    fs.closeSync(fd)
  }
}
