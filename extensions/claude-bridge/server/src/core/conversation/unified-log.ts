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

type Entry = JsonlEntry & { uuid?: string; parentUuid?: string | null; subtype?: string }

export const ENGINE_SWITCH_SUBTYPE = 'engine_switch'

/** Last main-chain uuid in a transcript file (sub-agent side chains skipped). */
export function lastUuidIn(file: string): string | null {
  let raw: string
  try {
    raw = fs.readFileSync(file, 'utf8')
  } catch {
    return null
  }
  const lines = raw.trimEnd().split('\n')
  for (let i = lines.length - 1; i >= 0; i--) {
    try {
      const e = JSON.parse(lines[i]!) as { uuid?: unknown; isSidechain?: unknown }
      if (typeof e.uuid === 'string' && e.uuid && !e.isSidechain) return e.uuid
    } catch {
      /* torn line */
    }
  }
  return null
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
  let raw: string
  try {
    raw = fs.readFileSync(file, 'utf8')
  } catch {
    return out
  }
  for (const m of raw.matchAll(/"uuid":"([^"]+)"/g)) out.add(m[1]!)
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
        if (!e.isSidechain) this.last = e.uuid
      }
      lines.push(JSON.stringify(e))
    }
    if (lines.length === 0) return
    fs.mkdirSync(path.dirname(this.file), { recursive: true })
    fs.appendFileSync(this.file, lines.join('\n') + '\n')
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
  let data = ''
  try {
    data = fs.readFileSync(source, 'utf8')
  } catch {
    /* an empty chat has no transcript yet */
  }
  // A torn last line (process killed mid-write) is dropped, not copied.
  const end = data.lastIndexOf('\n')
  fs.writeFileSync(file, end >= 0 ? data.slice(0, end + 1) : '', { mode: 0o600 })
}
