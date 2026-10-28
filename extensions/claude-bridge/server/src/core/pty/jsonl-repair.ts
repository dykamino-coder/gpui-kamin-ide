// ============================================================================
// Resume-time transcript repair, bounded to the transcript TAIL.
//
// Every corruption this repair exists for is produced at the END of the file:
//  - a mid-write kill leaves a torn (unparseable) last line, and entries
//    written after it may point at the torn entry's uuid;
//  - an interrupted stream persists an `assistant` with `stop_reason: null`,
//    after which the CLI appends `<synthetic>` placeholders whose message.id is
//    a UUID, not a `msg_*` id. The next request then sends that id as
//    `previous_message_id` and the API rejects it forever.
//
// The previous implementation read, split and JSON-parsed the WHOLE transcript
// synchronously on the server's only JS thread, on every resume — 0.3–1 s of a
// frozen server per 70–177 MiB transcript, with several full copies in memory —
// and copied the whole file as a backup before writing it back in full.
//
// This version reads a bounded window from the end (asynchronously, so the
// event loop keeps serving), analyses only that window, and rewrites only the
// bytes from the first changed line onward:
//   1. drop unparseable lines and `<synthetic>`-id entries;
//   2. re-parent entries that pointed at a dropped entry to that entry's own
//      (resolved) parent, which is the nearest preceding kept entry in the
//      chain;
//   3. truncate after the last completed real assistant / compaction
//      checkpoint when a broken assistant follows it.
// When step 3 needs an anchor that is older than the window, the window grows
// (bounded); past the bound the tail is left alone rather than guessed.
//
// Safety: the original bytes of the rewritten range are backed up first; the
// rewrite is skipped if the file changed while it was analysed (another writer
// appended), so a concurrent append can never be overwritten.
// ============================================================================

import fsp from 'fs/promises'
import { debugLog, warnLog } from '../logging'
import { readRange, yieldToEventLoop } from './jsonl-lines'

export interface JsonlRepairStats {
  repaired: boolean
  droppedUnparseable: number
  droppedSynthetic: number
  relinkedParents: number
  truncatedTail: number
}

export const NO_REPAIR: JsonlRepairStats = Object.freeze({
  repaired: false,
  droppedUnparseable: 0,
  droppedSynthetic: 0,
  relinkedParents: 0,
  truncatedTail: 0,
}) as JsonlRepairStats

export interface TailRepairOptions {
  /** Initial tail window. */
  windowBytes?: number
  /** Upper bound the window may grow to while looking for a truncation anchor. */
  maxWindowBytes?: number
}

const DEFAULT_WINDOW = 4 * 1024 * 1024
const DEFAULT_MAX_WINDOW = 16 * 1024 * 1024

interface Line {
  start: number
  end: number // just past LF (or EOF)
  obj: Record<string, unknown> | null
  rewritten: string | null
  drop: boolean
}

type Entry = {
  type?: string
  subtype?: string
  uuid?: unknown
  parentUuid?: unknown
  isCompactSummary?: unknown
  message?: { id?: unknown; model?: unknown; stop_reason?: unknown; content?: unknown }
}

const UUID_RE = /"uuid"\s*:\s*"([^"]+)"/
const PARENT_RE = /"parentUuid"\s*:\s*"([^"]+)"/

/** Repair a transcript tail before `--resume`. Never reads the whole file. */
export async function repairJsonlTail(jsonlPath: string, opts: TailRepairOptions = {}): Promise<JsonlRepairStats> {
  let st
  try {
    st = await fsp.stat(jsonlPath)
  } catch {
    return NO_REPAIR
  }
  if (st.size === 0) return NO_REPAIR
  const maxWindow = Math.max(opts.maxWindowBytes ?? DEFAULT_MAX_WINDOW, opts.windowBytes ?? DEFAULT_WINDOW)
  let window = Math.min(st.size, opts.windowBytes ?? DEFAULT_WINDOW)
  for (;;) {
    const winStart = st.size - window
    const buf = await readRange(jsonlPath, winStart, st.size)
    const canGrow = winStart > 0 && window < maxWindow
    const plan = await analyseWindow(buf, winStart, canGrow)
    if (plan === 'grow') {
      window = Math.min(st.size, maxWindow, window * 4)
      continue
    }
    if (plan === null) return NO_REPAIR
    return applyPlan(jsonlPath, st, buf, winStart, plan)
  }
}

interface Plan {
  changeStart: number // absolute byte offset of the first rewritten line
  replacement: Buffer
  stats: JsonlRepairStats
}

/** Decide the rewrite for a tail window. `null` = nothing to change;
 *  `'grow'` = the decision needs bytes older than the window. */
export async function analyseWindow(buf: Buffer, winStart: number, canGrow = true): Promise<Plan | null | 'grow'> {
  // A window that does not start at the file start begins mid-line: skip the
  // partial first line, its bytes belong to a record we cannot see whole.
  let off = 0
  if (winStart > 0) {
    const nl = buf.indexOf(0x0a)
    if (nl === -1) return canGrow ? 'grow' : null
    off = nl + 1
  }
  const lines: Line[] = []
  let sinceYield = 0
  while (off < buf.length) {
    let nl = buf.indexOf(0x0a, off)
    const end = nl === -1 ? buf.length : nl + 1
    if (nl === -1) nl = buf.length
    const text = buf.toString('utf8', off, nl)
    if (text.trim().length > 0) {
      let obj: Record<string, unknown> | null
      try {
        const v = JSON.parse(text) as unknown
        obj = v && typeof v === 'object' ? (v as Record<string, unknown>) : null
      } catch {
        obj = null
      }
      lines.push({ start: winStart + off, end: winStart + end, obj, rewritten: null, drop: false })
    }
    sinceYield += end - off
    off = end
    // Parsing a multi-MiB window must not freeze the shared event loop.
    if (sinceYield >= 256 * 1024) {
      sinceYield = 0
      await yieldToEventLoop()
    }
  }
  if (lines.length === 0) return null

  let droppedUnparseable = 0
  let droppedSynthetic = 0
  // dropped uuid → the parent it pointed at (so its children can be re-parented).
  const droppedParent = new Map<string, string | null>()
  let lastKeptUuid: string | null = null
  const rawText = (l: Line): string => buf.toString('utf8', l.start - winStart, l.end - winStart)

  for (const l of lines) {
    if (l.obj === null) {
      l.drop = true
      droppedUnparseable++
      // A torn line may still carry its uuid/parent at the front.
      const raw = rawText(l)
      const u = UUID_RE.exec(raw)?.[1]
      if (u) droppedParent.set(u, PARENT_RE.exec(raw)?.[1] ?? lastKeptUuid)
      continue
    }
    const e = l.obj as Entry
    const mid = e.message?.id
    if (typeof mid === 'string' && mid.length > 0 && !mid.startsWith('msg_') && !isCompactCheckpoint(e)) {
      l.drop = true
      droppedSynthetic++
      if (typeof e.uuid === 'string' && e.uuid) {
        droppedParent.set(e.uuid, typeof e.parentUuid === 'string' ? e.parentUuid : null)
      }
      continue
    }
    if (typeof e.uuid === 'string' && e.uuid) lastKeptUuid = e.uuid
  }

  let relinkedParents = 0
  for (const l of lines) {
    if (l.drop || !l.obj) continue
    const e = l.obj as Entry
    if (typeof e.parentUuid !== 'string' || !droppedParent.has(e.parentUuid)) continue
    let p: string | null = e.parentUuid
    const seen = new Set<string>()
    while (p !== null && droppedParent.has(p) && !seen.has(p)) {
      seen.add(p)
      p = droppedParent.get(p) ?? null
    }
    e.parentUuid = p
    l.rewritten = JSON.stringify(l.obj)
    relinkedParents++
  }

  // Tail truncation anchored at the last completed assistant or checkpoint.
  const kept = lines.filter((l) => !l.drop)
  let lastGood = -1
  for (let i = kept.length - 1; i >= 0; i--) {
    const e = kept[i]!.obj as Entry
    if (isCompletedRealAssistant(e) || isCompactCheckpoint(e)) {
      lastGood = i
      break
    }
  }
  let truncatedTail = 0
  let truncateFrom: number | null = null
  const hasBrokenAfter = (from: number): boolean => {
    for (let i = from; i < kept.length; i++) if (isBrokenAssistant(kept[i]!.obj as Entry)) return true
    return false
  }
  if (lastGood >= 0) {
    if (hasBrokenAfter(lastGood + 1)) {
      truncatedTail = kept.length - lastGood - 1
      truncateFrom = kept[lastGood + 1]!.start
    }
  } else if (winStart > 0 && canGrow) {
    // The anchor is older than this window, and a broken assistant may sit
    // between it and the window start: decide on a wider window. Past the
    // bound only the certain repairs (torn/synthetic lines) are applied.
    return 'grow'
  }

  const changed = droppedUnparseable > 0 || droppedSynthetic > 0 || relinkedParents > 0 || truncatedTail > 0
  if (!changed) return null

  let changeStart = Infinity
  for (const l of lines) {
    if (l.drop || l.rewritten !== null) {
      changeStart = l.start
      break
    }
  }
  if (truncateFrom !== null) changeStart = Math.min(changeStart, truncateFrom)
  const parts: Buffer[] = []
  for (const l of lines) {
    if (l.start < changeStart || l.drop) continue
    if (truncateFrom !== null && l.start >= truncateFrom) break
    if (l.rewritten !== null) parts.push(Buffer.from(l.rewritten + '\n', 'utf8'))
    else {
      const raw = buf.subarray(l.start - winStart, l.end - winStart)
      parts.push(raw)
      if (raw[raw.length - 1] !== 0x0a) parts.push(Buffer.from('\n'))
    }
  }
  return {
    changeStart,
    replacement: Buffer.concat(parts),
    stats: { repaired: true, droppedUnparseable, droppedSynthetic, relinkedParents, truncatedTail },
  }
}

async function applyPlan(
  jsonlPath: string,
  st: { size: number; mtimeMs: number },
  buf: Buffer,
  winStart: number,
  plan: Plan,
): Promise<JsonlRepairStats> {
  // Back up exactly the bytes being replaced; the prefix is not touched.
  const backup = `${jsonlPath}.bak-${Date.now()}-tail-${plan.changeStart}`
  try {
    await fsp.writeFile(backup, buf.subarray(plan.changeStart - winStart))
  } catch (e) {
    warnLog('repairJsonl backup failed; skipping repair to avoid data loss', { jsonlPath, error: String(e) })
    return NO_REPAIR
  }
  const fh = await fsp.open(jsonlPath, 'r+')
  try {
    // Another writer appended (or rewrote) since the analysis: never clobber it.
    const now = await fh.stat()
    if (now.size !== st.size || now.mtimeMs !== st.mtimeMs) {
      warnLog('repairJsonl skipped: transcript changed during analysis', { jsonlPath })
      await fsp.rm(backup, { force: true })
      return NO_REPAIR
    }
    if (plan.replacement.length > 0) await fh.write(plan.replacement, 0, plan.replacement.length, plan.changeStart)
    await fh.truncate(plan.changeStart + plan.replacement.length)
    await fh.datasync()
  } finally {
    await fh.close()
  }
  debugLog('repairJsonl: transcript tail repaired', { jsonlPath, ...plan.stats })
  return plan.stats
}

/** True iff the entry came from a real /v1/messages response that completed. */
export function isCompletedRealAssistant(e: Entry | null): boolean {
  if (!e || e.type !== 'assistant') return false
  const m = e.message
  if (!m) return false
  if (typeof m.id !== 'string' || !m.id.startsWith('msg_')) return false
  if (m.model === '<synthetic>') return false
  // stop_reason: null means the stream was interrupted before message_stop.
  if (m.stop_reason === null || m.stop_reason === undefined) return false
  return true
}

/** A CLI-written compaction checkpoint: legitimate, resumable state that the
 *  repair must never treat as corruption or truncate across. Detects every
 *  on-disk shape: the `compact_boundary` system entry, `type:"compact-summary"`,
 *  the `isCompactSummary` flag, and the literal summary marker text some CLI
 *  versions write without the flag. */
export function isCompactCheckpoint(e: Entry | null): boolean {
  if (!e || typeof e !== 'object') return false
  if (e.type === 'system' && e.subtype === 'compact_boundary') return true
  if (e.type === 'compact-summary') return true
  if (e.isCompactSummary === true) return true
  const c = e.message?.content
  const text =
    typeof c === 'string' ? c : Array.isArray(c) ? c.map((b) => (b as { text?: string }).text ?? '').join(' ') : ''
  return /compacted\s*\(ctrl[+\s]*o/i.test(text)
}

/** True iff the assistant entry would break the previous_message_id chain. */
export function isBrokenAssistant(e: Entry | null): boolean {
  if (!e || e.type !== 'assistant') return false
  const m = e.message
  if (!m) return false
  if (m.model === '<synthetic>') return true
  if (typeof m.id !== 'string' || !m.id.startsWith('msg_')) return true
  if (m.stop_reason === null) return true
  return false
}
