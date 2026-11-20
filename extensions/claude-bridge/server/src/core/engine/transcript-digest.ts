// ============================================================================
// Conversation digest: one sidecar file next to a transcript (`<log>.idx`)
// that the server keeps up to date as the log grows, so nothing has to read
// the whole log again.
//
// It holds, each with a pointer into the log (line number, byte offset, entry
// uuid):
//   - every STRIDE-th line start: the line index `from_index` and `tail` seek
//     through (project-sessions/line-index.ts);
//   - every user request, compaction boundary and engine switch, and turn and
//     byte counts per UTC hour: project_session_outline lists them;
//   - checkpoints of the engine-switch handoff digest (handoff.ts TurnDigest):
//     a switch restores the last one and reads only the lines after it, so its
//     cost does not depend on the conversation's length.
//
// The file is append-only JSON lines. A checkpoint names the last line it
// covers (offset, length and a hash of its bytes). On first use in a process
// the newest checkpoint whose line still matches the log is the start point:
// records after it are cut and rebuilt from the log. A torn record (a crash
// mid-write), a torn log tail, a repaired or rewritten log or a reseeded copy
// therefore all resume from the last point that still holds, not from zero; a
// log without a sidecar is built once, in chunks, with the event loop free.
// ============================================================================

import { createHash } from 'node:crypto'
import fs from 'node:fs'
import fsp from 'node:fs/promises'
import type { JsonlEntry } from '../../shared/jsonl-types'
import { warnLog } from '../logging'
import {
  HANDOFF_BUDGET_BYTES,
  TurnDigest,
  fromDigest,
  withoutContextBlocks,
  type DigestState,
  type Handoff,
  type HandoffSource,
} from './handoff'

export const DIGEST_SUFFIX = '.idx'
const VERSION = 1
/** Line index stride (one offset per STRIDE lines). */
export const DIGEST_STRIDE = 1024
/** Log bytes between handoff checkpoints: the most a switch re-reads. */
let CHECKPOINT_BYTES = 4 * 1024 * 1024
const READ_CHUNK = 1024 * 1024
const REQUEST_CHARS = 200
const CACHE_LIMIT = 32
const APPEND_DELAY_MS = 1_000

export interface Pointer {
  /** Line number in the log (the index every chat tool uses). */
  line: number
  /** Byte offset of the line. */
  offset: number
  uuid: string | null
  at: string | null
}
export interface RequestPointer extends Pointer {
  text: string
}
export interface CompactionPointer extends Pointer {
  trigger: string | null
  preTokens: number | null
}
export interface SegmentPointer extends Pointer {
  engine: string | null
  model: string | null
}
export interface PeriodCount {
  /** UTC hour, "YYYY-MM-DDTHH". */
  hour: string
  turns: number
  bytes: number
  /** Where the hour's first entry is. */
  line: number
  offset: number
}

type Rec =
  | { k: 'h'; v: number; budget: number; stride: number }
  | { k: 'l'; n: number; o: number }
  | ({ k: 'u' } & RequestPointer)
  | ({ k: 'c' } & CompactionPointer)
  | ({ k: 's' } & SegmentPointer)
  | ({ k: 'p' } & PeriodCount)
  | {
      k: 'z'
      /** Lines and bytes covered; the last covered line starts at `o`, ends at `end`. */
      n: number
      end: number
      o: number
      h: string
      st: DigestState
      period: PeriodCount | null
    }

export interface Digest {
  readonly file: string
  /** Bytes of complete lines covered, and their count. */
  end: number
  lines: number
  /** offsets[k] = first byte of line k * DIGEST_STRIDE. */
  offsets: number[]
  requests: RequestPointer[]
  compactions: CompactionPointer[]
  segments: SegmentPointer[]
  periods: PeriodCount[]
  /** The open hour (not yet in `periods`). */
  period: PeriodCount | null
  handoff: TurnDigest
  /** Identity, size and mtime of the log at the last catch-up. */
  identity: string
  size: number
  mtimeMs: number
  lastStart: number
  lastHash: string
  sinceCheckpoint: number
  /** The sidecar holds every record of this digest (false: built by a read-only caller, in memory only). */
  persisted: boolean
}

/** Work done, for tests and the bench. */
export const digestStats = { builds: 0, resumes: 0, bytesParsed: 0, recordsWritten: 0 }

const cache = new Map<string, Digest>()
const running = new Map<string, Promise<Digest | null>>()
const timers = new Map<string, NodeJS.Timeout>()

export const sidecarOf = (file: string) => file + DIGEST_SUFFIX
const hashOf = (b: Buffer) => createHash('sha1').update(b).digest('hex').slice(0, 20)
const yieldLoop = () => new Promise<void>((resolve) => setImmediate(resolve))

function fresh(file: string, budget: number): Digest {
  return {
    file,
    end: 0,
    lines: 0,
    offsets: [0],
    requests: [],
    compactions: [],
    segments: [],
    periods: [],
    period: null,
    handoff: new TurnDigest(budget),
    identity: '',
    size: 0,
    mtimeMs: 0,
    lastStart: 0,
    lastHash: '',
    sinceCheckpoint: 0,
    persisted: true,
  }
}

async function readSidecar(file: string): Promise<{ recs: Rec[]; ends: number[] }> {
  let buf: Buffer
  try {
    buf = await fsp.readFile(sidecarOf(file))
  } catch {
    return { recs: [], ends: [] }
  }
  const recs: Rec[] = []
  const ends: number[] = []
  let pos = 0
  for (let nl = buf.indexOf(10); nl !== -1; nl = buf.indexOf(10, pos)) {
    try {
      recs.push(JSON.parse(buf.toString('utf8', pos, nl)) as Rec)
      ends.push(nl + 1)
    } catch {
      break // a torn record: everything after it is rebuilt
    }
    pos = nl + 1
  }
  return { recs, ends }
}

async function lineMatches(fh: fsp.FileHandle, size: number, z: Extract<Rec, { k: 'z' }>): Promise<boolean> {
  if (z.end > size || z.o >= z.end) return false
  const buf = Buffer.allocUnsafe(z.end - z.o)
  const { bytesRead } = await fh.read(buf, 0, buf.length, z.o)
  return bytesRead === buf.length && buf[buf.length - 1] === 10 && hashOf(buf) === z.h
}

/** The digest at the newest checkpoint that still matches the log; the
 *  sidecar is cut back to it. */
async function load(file: string, fh: fsp.FileHandle, size: number, budget: number, persist: boolean): Promise<Digest> {
  const { recs, ends } = await readSidecar(file)
  const head = recs[0]
  let keep = -1
  if (head?.k === 'h' && head.v === VERSION && head.budget === budget && head.stride === DIGEST_STRIDE) {
    for (let i = recs.length - 1; i > 0; i--) {
      const r = recs[i]!
      if (r.k === 'z' && (await lineMatches(fh, size, r))) {
        keep = i
        break
      }
    }
  }
  const d = fresh(file, budget)
  d.persisted = persist
  if (keep === -1) {
    const header: Rec = { k: 'h', v: VERSION, budget, stride: DIGEST_STRIDE }
    if (persist) await fsp.writeFile(sidecarOf(file), JSON.stringify(header) + '\n', { mode: 0o600 })
    digestStats.builds++
    return d
  }
  for (let i = 1; i <= keep; i++) {
    const r = recs[i]!
    if (r.k === 'l') d.offsets.push(r.o)
    else if (r.k === 'u') d.requests.push(strip(r))
    else if (r.k === 'c') d.compactions.push(strip(r))
    else if (r.k === 's') d.segments.push(strip(r))
    else if (r.k === 'p') d.periods.push(strip(r))
  }
  const z = recs[keep] as Extract<Rec, { k: 'z' }>
  d.end = z.end
  d.lines = z.n
  d.lastStart = z.o
  d.lastHash = z.h
  d.period = z.period
  d.handoff = TurnDigest.restore(z.st)
  d.sinceCheckpoint = z.end
  if (persist && ends[keep]! < (await fsp.stat(sidecarOf(file))).size) await fsp.truncate(sidecarOf(file), ends[keep]!)
  digestStats.resumes++
  return d
}

function strip<T extends { k: string }>(r: T): Omit<T, 'k'> {
  const { k: _k, ...rest } = r
  return rest
}

const ts = (e: { timestamp?: unknown }) => (typeof e.timestamp === 'string' ? e.timestamp : null)
const uuidOf = (e: { uuid?: unknown }) => (typeof e.uuid === 'string' && e.uuid ? e.uuid : null)

/** The text of a user's own request (not a tool result, meta line or side chain). */
function requestText(e: JsonlEntry): string | null {
  if (e.type !== 'user' || e.isSidechain || e.isMeta) return null
  const c = e.message?.content
  const raw =
    typeof c === 'string'
      ? c
      : Array.isArray(c)
        ? c
            .filter((b) => b.type === 'text')
            .map((b) => (b as { text: string }).text)
            .join('\n')
        : ''
  const t = withoutContextBlocks(raw).replace(/\s+/g, ' ').trim()
  return t ? (t.length > REQUEST_CHARS ? `${t.slice(0, REQUEST_CHARS)}…` : t) : null
}

/** Feed one complete line; returns the records it adds. */
function feed(d: Digest, raw: Buffer, start: number, out: Rec[]): void {
  const n = d.lines++
  d.end = start + raw.length + 1
  if (d.lines % DIGEST_STRIDE === 0) {
    d.offsets.push(d.end)
    out.push({ k: 'l', n: d.lines, o: d.end })
  }
  const text = raw.toString('utf8')
  if (!text.trim()) return
  let e: JsonlEntry & Record<string, unknown>
  try {
    e = JSON.parse(text)
  } catch {
    return
  }
  if (!e || typeof e !== 'object') return
  const turns = d.handoff.total
  d.handoff.add(e, n)
  const at = ts(e)
  const hour = at && /^\d{4}-\d\d-\d\dT\d\d/.test(at) ? at.slice(0, 13) : (d.period?.hour ?? null)
  if (hour) {
    if (d.period && d.period.hour !== hour) {
      d.periods.push(d.period)
      out.push({ k: 'p', ...d.period })
      d.period = null
    }
    d.period ??= { hour, turns: 0, bytes: 0, line: n, offset: start }
    d.period.turns += d.handoff.total - turns
    d.period.bytes += raw.length + 1
  }
  const ptr = { line: n, offset: start, uuid: uuidOf(e), at }
  const request = requestText(e)
  if (request) {
    const r = { ...ptr, text: request }
    d.requests.push(r)
    out.push({ k: 'u', ...r })
  } else if (e.type === 'system' && e.subtype === 'compact_boundary') {
    const meta = e.compactMetadata as { trigger?: unknown; preTokens?: unknown } | undefined
    const r = {
      ...ptr,
      trigger: typeof meta?.trigger === 'string' ? meta.trigger : null,
      preTokens: typeof meta?.preTokens === 'number' ? meta.preTokens : null,
    }
    d.compactions.push(r)
    out.push({ k: 'c', ...r })
  } else if (e.type === 'system' && e.subtype === 'engine_switch') {
    const r = {
      ...ptr,
      engine: typeof e.engine === 'string' ? e.engine : null,
      model: typeof e.model === 'string' ? e.model : null,
    }
    d.segments.push(r)
    out.push({ k: 's', ...r })
  }
}

/** Read the log from `d.end` to its last newline, feeding the digest and
 *  appending records (a checkpoint every CHECKPOINT_BYTES). */
async function catchUp(d: Digest, fh: fsp.FileHandle, size: number): Promise<void> {
  const buf = Buffer.allocUnsafe(READ_CHUNK)
  let carry: Buffer[] = []
  let carryStart = d.end
  let pos = d.end
  const out: Rec[] = []
  const flush = async () => {
    if (!out.length) return
    if (!d.persisted) {
      out.length = 0
      return
    }
    digestStats.recordsWritten += out.length
    await fsp.appendFile(sidecarOf(d.file), out.map((r) => JSON.stringify(r)).join('\n') + '\n')
    out.length = 0
  }
  while (pos < size) {
    const { bytesRead } = await fh.read(buf, 0, Math.min(READ_CHUNK, size - pos), pos)
    if (bytesRead === 0) break
    digestStats.bytesParsed += bytesRead
    let from = 0
    for (let nl = buf.indexOf(10, 0); nl !== -1 && nl < bytesRead; nl = buf.indexOf(10, from)) {
      const piece = buf.subarray(from, nl)
      const raw = carry.length ? Buffer.concat([...carry, piece]) : piece
      const start = carryStart
      feed(d, raw, start, out)
      d.lastStart = start
      if (d.end - d.sinceCheckpoint >= CHECKPOINT_BYTES) {
        out.push(checkpoint(d, raw))
        await flush()
      }
      carry = []
      from = nl + 1
      carryStart = pos + from
    }
    if (from < bytesRead) carry.push(Buffer.from(buf.subarray(from, bytesRead)))
    pos += bytesRead
    await yieldLoop()
  }
  await flush()
}

function checkpoint(d: Digest, raw: Buffer): Rec {
  d.lastHash = hashOf(Buffer.concat([raw, Buffer.from('\n')]))
  d.sinceCheckpoint = d.end
  return {
    k: 'z',
    n: d.lines,
    end: d.end,
    o: d.lastStart,
    h: d.lastHash,
    st: d.handoff.snapshot(),
    period: d.period ? { ...d.period } : null,
  }
}

/** A checkpoint at the current end, when the last one is far enough behind
 *  that the next process would re-read much (the end of a lazy build). */
async function closeOut(d: Digest, fh: fsp.FileHandle, minBytes: number): Promise<void> {
  if (d.end - d.sinceCheckpoint < minBytes || d.end === 0) return
  const raw = Buffer.allocUnsafe(d.end - d.lastStart - 1)
  await fh.read(raw, 0, raw.length, d.lastStart)
  const z = checkpoint(d, raw)
  if (!d.persisted) return
  digestStats.recordsWritten++
  await fsp.appendFile(sidecarOf(d.file), JSON.stringify(z) + '\n')
}

async function update(file: string, budget: number, persist: boolean): Promise<Digest | null> {
  let fh: fsp.FileHandle
  try {
    fh = await fsp.open(file, 'r')
  } catch {
    if (persist) forgetDigest(file)
    else cache.delete(file)
    return null
  }
  try {
    const st = await fh.stat()
    const identity = `${st.dev}:${st.ino}:${st.birthtimeMs}`
    let d = cache.get(file)
    if (d && (d.handoff.budget !== budget || (persist && !d.persisted))) d = undefined
    if (d && d.identity === identity && d.size === st.size && d.mtimeMs === st.mtimeMs) return touch(d)
    // A known digest still holds when the log only grew past its end and its
    // last line is unchanged; anything else goes through the checkpoints.
    if (d && !(st.size >= d.end && d.identity === identity && (await lastLineHolds(fh, d)))) d = undefined
    const built = !d
    d ??= await load(file, fh, st.size, budget, persist)
    const before = d.end
    await catchUp(d, fh, st.size)
    if (d.end !== before) {
      const last = Buffer.allocUnsafe(d.end - d.lastStart)
      await fh.read(last, 0, last.length, d.lastStart)
      d.lastHash = hashOf(last)
    }
    await closeOut(d, fh, built ? 1 : CHECKPOINT_BYTES)
    d.identity = identity
    d.size = st.size
    d.mtimeMs = st.mtimeMs
    return touch(d)
  } finally {
    await fh.close()
  }
}

async function lastLineHolds(fh: fsp.FileHandle, d: Digest): Promise<boolean> {
  if (d.end === 0) return true
  const buf = Buffer.allocUnsafe(d.end - d.lastStart)
  const { bytesRead } = await fh.read(buf, 0, buf.length, d.lastStart)
  if (bytesRead !== buf.length || buf[buf.length - 1] !== 10) return false
  return hashOf(buf) === d.lastHash
}

function touch(d: Digest): Digest {
  cache.delete(d.file)
  cache.set(d.file, d)
  if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value!)
  return d
}

/**
 * The digest of a log, brought up to date with it (only the bytes appended
 * since are read). Null when the log does not exist. Calls for one file run
 * one at a time. `persist: false` (the read-only chat tools) never writes:
 * it uses the sidecar as it is and keeps the rest in memory.
 */
export function digestOf(
  file: string,
  budget = HANDOFF_BUDGET_BYTES,
  { persist = true }: { persist?: boolean } = {},
): Promise<Digest | null> {
  const prev = running.get(file) ?? Promise.resolve(null)
  const next = prev
    .catch(() => null)
    .then(() => update(file, budget, persist))
    .catch((err) => {
      warnLog('Conversation digest: update failed', { file, error: String(err) })
      cache.delete(file)
      return null
    })
  running.set(file, next)
  void next.finally(() => {
    if (running.get(file) === next) running.delete(file)
  })
  return next
}

/** The digest as last brought up to date, without touching the disk. */
export function peekDigest(file: string): Digest | null {
  return cache.get(file) ?? null
}

/** The log grew: bring its digest along shortly (appends close together are
 *  read in one pass). */
export function noteAppended(file: string): void {
  if (timers.has(file)) return
  const t = setTimeout(() => {
    timers.delete(file)
    void digestOf(file)
  }, APPEND_DELAY_MS)
  t.unref?.()
  timers.set(file, t)
}

/** The log is gone (deleted chat): drop its digest and sidecar. */
export function forgetDigest(file: string): void {
  cache.delete(file)
  const t = timers.get(file)
  if (t) clearTimeout(t)
  timers.delete(file)
  try {
    fs.rmSync(sidecarOf(file), { force: true })
  } catch {
    /* already gone */
  }
}

/** A log that starts as a byte copy of another (first switch): its sidecar
 *  starts as a copy too, and is checked against the copy on first use. */
export function copyDigest(from: string, to: string): void {
  try {
    fs.copyFileSync(sidecarOf(from), sidecarOf(to))
    fs.chmodSync(sidecarOf(to), 0o600)
  } catch {
    /* none yet: built on first use */
  }
}

/**
 * The switch handoff of a log, from its digest: the last checkpoint plus the
 * lines after it. Same block as handoff.ts `readHandoff` (which reads the
 * whole log).
 */
export async function handoffFromLog(
  file: string,
  source: HandoffSource,
  options: { budgetBytes?: number } = {},
): Promise<Handoff | null> {
  const budget = options.budgetBytes ?? HANDOFF_BUDGET_BYTES
  const d = await digestOf(file, budget)
  const h = d ? d.handoff.clone() : new TurnDigest(budget)
  if (d && d.size > d.end) {
    // A last line without its newline yet: counted for this block only.
    try {
      const fh = await fsp.open(file, 'r')
      try {
        const buf = Buffer.allocUnsafe(d.size - d.end)
        const { bytesRead } = await fh.read(buf, 0, buf.length, d.end)
        const text = buf.toString('utf8', 0, bytesRead)
        if (text.trim()) h.add(JSON.parse(text) as JsonlEntry, d.lines)
      } finally {
        await fh.close()
      }
    } catch {
      /* torn line */
    }
  }
  return fromDigest(h.finish(), source, budget)
}

/** Tests: checkpoints closer together. */
export function setCheckpointBytesForTests(n: number): void {
  CHECKPOINT_BYTES = n
}

/** Tests: forget every digest in memory (as a server restart would). */
export function resetDigestsForTests(): void {
  cache.clear()
  running.clear()
  for (const t of timers.values()) clearTimeout(t)
  timers.clear()
  digestStats.builds = 0
  digestStats.resumes = 0
  digestStats.bytesParsed = 0
  digestStats.recordsWritten = 0
}
