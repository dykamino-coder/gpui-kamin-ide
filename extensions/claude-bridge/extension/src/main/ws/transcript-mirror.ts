// A local copy of a session's transcript, so restarting the IDE doesn't mean
// pulling the whole thing over the network again.
//
// Measured on a real session: 85MB / 30 971 records took 15–20s from the server,
// against 286ms to read the same bytes from disk and 509ms to parse ALL of them
// (far less for the tail, which is all the chat needs to paint). The mirror
// turns "download everything, every launch" into "read locally, ask only for
// what arrived since".
//
// Correctness rests on the cursor being meaningful. Records carry `_pos` — their
// byte offset in the server's file — which is stable across replays, unlike
// `_ord`, a per-process counter the server re-tags on every read. Alongside the
// records we store the file's fingerprint; if the server reports a different
// one, the transcript was rewritten (the CLI compacts by rewriting) and the
// mirror is NOT a prefix of it. Extending it then would splice new records onto
// stale ones and fabricate a conversation that never happened, so that case
// throws the mirror away instead.
import { createWriteStream, type WriteStream } from 'fs'
import fsp from 'fs/promises'
import path from 'path'

/** Максимальный допустимый разрыв между курсором и началом append-батча.
 *  Живой хвост примыкает к курсору с точностью до пропущенных torn-строк
 *  (байты); больший разрыв = out-of-order реплей, который зеркалу нельзя. */
const MAX_APPEND_GAP_BYTES = 256 * 1024

export interface MirrorCursor {
  /** Fingerprint of the server file this mirror was built from. */
  head: string
  /** Bytes of the server file covered by this mirror. */
  pos: number
  /** uuid of the last record — a second, independent check that the server's
   *  view and ours agree at that offset. */
  lastUuid?: string
  lastPos?: number
}

// v3 preserves physical file order. Older timestamp-sorted mirrors cannot be
// searched by server byte position; replace them with a bounded tail snapshot.
interface Sidecar extends MirrorCursor {
  v: 3
  /** Local mirror size (bytes) at sidecar write — `pos` is the SERVER offset,
   *  so it can't validate the local file. A mirror shorter than this means we
   *  died mid-write and lost a tail the cursor claims to hold (audit #70 C9).
   *  Absent on legacy sidecars → skip the check. */
  size?: number
}

export class TranscriptMirror {
  private readonly file: string
  private readonly sidecar: string
  private stream: WriteStream | null = null
  private cursor: MirrorCursor | null = null
  /** Writes are chained so a burst of arriving batches cannot interleave. */
  private queue: Promise<void> = Promise.resolve()

  constructor(dir: string, conversationId: string) {
    this.file = path.join(dir, `${conversationId}.jsonl`)
    this.sidecar = path.join(dir, `${conversationId}.cursor.json`)
  }

  /** What we can prove we already hold. `null` = nothing usable. */
  async readCursor(): Promise<MirrorCursor | null> {
    if (this.cursor) return this.cursor
    try {
      const raw = await fsp.readFile(this.sidecar, 'utf8')
      const parsed = JSON.parse(raw) as Sidecar
      if (parsed.v !== 3 || typeof parsed.pos !== 'number' || !parsed.head) return null
      // A sidecar claiming more than the file holds means we died mid-write;
      // trusting it would ask the server to resume past records we never stored.
      const stat = await fsp.stat(this.file)
      if (stat.size === 0) return null
      // Local-size check (C9): mirror shorter than at sidecar write = torn
      // tail → the cursor lies about what we hold; full pull instead of a
      // permanent history hole. `>=` is fine — appends after the debounced
      // sidecar write only make the file longer.
      if (typeof parsed.size === 'number' && stat.size < parsed.size) return null
      this.cursor = { head: parsed.head, pos: parsed.pos, lastUuid: parsed.lastUuid, lastPos: parsed.lastPos }
      return this.cursor
    } catch {
      return null // no mirror yet, or unreadable — caller falls back to a full pull
    }
  }

  /** Read up to `count` records whose byte offset (`_pos`) is BEFORE `beforePos`,
   *  returned oldest→newest (ready to prepend above the current window). The
   *  foundation for scroll-up: the chat holds only a recent window in the webview
   *  heap, and pulls the preceding page from this disk copy on demand.
   *
   *  Server offsets refer to the source JSONL, not this serialized mirror.
   *  Locate the preceding local row by its stored source offset, then read a
   *  bounded byte window in the host process. */
  async readRange(beforePos: number, count: number): Promise<unknown[]> {
    const localEnd = await this.localPosition(beforePos)
    // Одна запись КРУПНЕЕ окна (жирный tool_result: зеркало хранит полные
    // записи, trimHeavyToolResults живёт только в webview-сторе) давала
    // пустую страницу — рендерер трактовал её как «начало файла» и навсегда
    // выключал скролл-ап (аудит #70 C10). Пустой результат при beforePos
    // больше окна → повторить с удвоенным окном, до 3 попыток.
    let window = Math.max(256 * 1024, count * 4096)
    for (let attempt = 0; attempt < 3; attempt++) {
      const out = await this.readRangeWindow(beforePos, count, window, localEnd)
      if (out.length > 0 || localEnd <= window) return out
      window *= 4
    }
    return []
  }

  private async readRangeWindow(beforePos: number, count: number, windowSize: number, localEnd: number): Promise<unknown[]> {
    if (beforePos <= 0 || count <= 0) return []
    // Sized to comfortably hold `count` records; the first (partial) line of a
    // mid-file read is dropped below.
    const window = Math.min(localEnd, windowSize)
    const start = localEnd - window
    let text: string
    try {
      const fh = await fsp.open(this.file, 'r')
      try {
        const buf = Buffer.alloc(window)
        const { bytesRead } = await fh.read(buf, 0, window, start)
        text = buf.subarray(0, bytesRead).toString('utf8')
      } finally { await fh.close() }
    } catch {
      return []
    }
    const lines = text.split('\n')
    if (start > 0 && lines.length) lines.shift() // partial leading line
    const out: unknown[] = []
    for (const line of lines) {
      if (!line) continue
      try {
        const r = JSON.parse(line) as { _pos?: number }
        // Only records strictly OLDER than the caller's boundary. A record with
        // no `_pos` can't be positioned, so it's skipped rather than risk a
        // duplicate or an out-of-order prepend.
        if (typeof r._pos !== 'number' || r._pos >= beforePos) continue
        out.push(r)
      } catch { /* torn line — skip */ }
    }
    // The `count` records closest to the boundary (the immediately-preceding
    // page), oldest→newest.
    return out.slice(-count)
  }

  /** Read the mirror back, newest records first — the chat paints its tail.
   *
   *  A BOUNDED read of a byte window ending at EOF, not the whole file: the chat
   *  needs only the last `maxRecords`, and on a marathon transcript (measured:
   *  85MB / 509ms to parse in full) reading + splitting the entire thing to keep
   *  the tail was a big synchronous stall for nothing. A window too small to hold
   *  `maxRecords` just returns fewer — the caller paints what it gets. */
  async readTail(maxRecords: number): Promise<unknown[]> {
    let size: number
    try { size = (await fsp.stat(this.file)).size } catch { return [] }
    if (size === 0 || maxRecords <= 0) return []
    const window = Math.min(size, Math.max(256 * 1024, maxRecords * 4096))
    const start = size - window
    let text: string
    try {
      const fh = await fsp.open(this.file, 'r')
      try {
        const buf = Buffer.alloc(window)
        const { bytesRead } = await fh.read(buf, 0, window, start)
        text = buf.subarray(0, bytesRead).toString('utf8')
      } finally { await fh.close() }
    } catch {
      return []
    }
    const lines = text.split('\n')
    if (start > 0 && lines.length) lines.shift() // drop the partial leading line
    const recs: { timestamp?: string }[] = []
    for (const line of lines) {
      if (!line) continue
      try { recs.push(JSON.parse(line) as { timestamp?: string }) } catch { /* torn line — skip */ }
    }
    // Records share a source fingerprint and retain physical file order.
    return recs.slice(-maxRecords)
  }

  /** Append records as they arrive. Never rewrites the file: an append-only
   *  mirror can be resumed after a crash, a rewritten one cannot. */
  append(records: readonly { _pos?: number; _posEnd?: number; uuid?: string }[], head: string): Promise<void> {
    if (records.length === 0) return this.queue
    this.queue = this.queue.then(async () => {
      // Only bytes BEYOND the cursor. Every resume replays the whole history
      // again, and appending it unfiltered duplicated the transcript per
      // resume — a real mirror was measured at 460MB for a ~20k-entry session,
      // and the disk churn of rewriting megabytes on every boot-time warm-pool
      // resume was a big part of the extension host's startup stall. A record
      // with no `_pos` can't be placed, so it's skipped rather than duplicated.
      const cur = this.cursor?.pos ?? 0
      const fresh = records
        .filter((r) => typeof r._pos === 'number' && r._pos >= cur)
        .sort((a, b) => (a._pos ?? 0) - (b._pos ?? 0))
      if (fresh.length === 0) return
      // Гейт примыкания: append принимает ТОЛЬКО хвост, продолжающий курсор.
      // Батч, начинающийся сильно дальше курсора, — это out-of-order реплей
      // (tail-preview шлёт КОНЕЦ файла первым): записать его = прыгнуть
      // курсором через недосланный промежуток, который затем навсегда
      // отсекается фильтром выше (прод-дыра «сообщение не отрисовалось»).
      // Основная защита — серверный флаг `replay` до этого вызова; гейт
      // страхует от старого сервера. Малый зазор допустим: сервер молча
      // пропускает torn-строки, это байты, не мегабайты.
      if ((fresh[0]!._pos ?? 0) - cur > MAX_APPEND_GAP_BYTES) return
      this.stream ??= createWriteStream(this.file, { flags: 'a' })
      const payload = fresh.map((r) => JSON.stringify(r)).join('\n') + '\n'
      await new Promise<void>((resolve, reject) => {
        this.stream!.write(payload, (err) => { err ? reject(err) : resolve() })
      })
      // Последняя ЗАПИСАННАЯ (fresh отсортирован по _pos) — не последняя в
      // батче: отфильтрованный «старый» хвост батча не должен решать курсор.
      const last = fresh[fresh.length - 1]!
      // The cursor is the boundary just PAST the last record (`_posEnd`), never
      // its start + 1: that lands one byte inside the record, and the server
      // then resumes mid-line and streams a truncated first record. Verified on
      // a real 30 971-record transcript — the off-by-one produced 10 972 records
      // where 10 971 were due, with no duplicate and no gap to give it away.
      //
      // A record without `_posEnd` (an older server) leaves the cursor where it
      // was: better to re-fetch a little than to resume from a guess.
      // The cursor advances only AFTER the bytes are handed to the stream, so a
      // crash leaves us claiming less than we hold — never more.
      this.cursor = {
        head,
        pos: last._posEnd ?? this.cursor?.pos ?? 0,
        lastUuid: last.uuid ?? this.cursor?.lastUuid,
        lastPos: last._pos,
      }
      // Дебаунс: writeFile+rename на КАЖДЫЙ батч — сотни пар fs-операций за
      // реплей (аудит #70 B8). Курсор в памяти всегда актуален; отставший на
      // секунду sidecar значит лишь чуть больший дозапрос при рестарте.
      this.scheduleSidecar()
    }).catch(() => { /* mirror is an optimisation: never break the session */ })
    return this.queue
  }

  /** Replace the mirror with a clean, in-FILE-ORDER copy of `records`.
   *
   *  Called at replay-complete with the full replayed history. It exists because
   *  the append path only runs once the file's identity (head) is known, which
   *  the server sends at replay-complete — so a RESUMED session's history (which
   *  streamed before that) was never mirrored, and the windowed store could evict
   *  it with no way to scroll it back. This writes that history down in one shot.
   *
   *  Sorted by `_pos`: the historical replay arrives newest-batch-first and out
   *  of order, but the mirror must be a valid prefix of the server file for
   *  `readRange` (which seeks by byte offset) to work. Fresh write, not append —
   *  a one-time reconciliation, after which live growth appends normally. The
   *  windowed store never asks for more than SCROLL_UP_MAX (< the ~20k cache this
   *  is fed), so capping the mirror to the cache costs the reader nothing. */
  async rewriteInOrder(records: readonly { _pos?: number; _posEnd?: number; uuid?: string }[], head: string): Promise<void> {
    // Dedup by uuid while ordering — a duplicate delivery upstream must not
    // become two lines on disk (which would double the message on scroll-up).
    const byUuid = new Map<string, { _pos?: number; _posEnd?: number; uuid?: string }>()
    const noUuid: { _pos?: number; _posEnd?: number; uuid?: string }[] = []
    for (const r of records) {
      if (typeof r._pos !== 'number') continue
      if (r.uuid) byUuid.set(r.uuid, r); else noUuid.push(r)
    }
    // Source positions preserve final replies with absent or skewed timestamps.
    const ordered = [...byUuid.values(), ...noUuid].sort(
      (a, b) => (a._pos ?? 0) - (b._pos ?? 0),
    )
    if (ordered.length === 0) return
    this.queue = this.queue.then(async () => {
      // Preserve earlier cached pages when the server identity still matches.
      // A different identity replaces the snapshot instead of mixing files.
      const existing = await this.readCursor()
      if (existing?.head === head) {
        const fresh = ordered.filter((r) => r._pos! >= Math.max(existing.pos, (existing.lastPos ?? -1) + 1))
        if (fresh.length === 0) return
        await this.close()
        await fsp.appendFile(this.file, fresh.map((r) => JSON.stringify(r)).join('\n') + '\n', 'utf8')
        const last = fresh.at(-1)!
        this.cursor = { head, pos: last._posEnd ?? existing.pos, lastUuid: last.uuid, lastPos: last._pos }
        await this.writeSidecar()
        return
      }
      await this.close() // drop any append stream — we're initialising the file
      const payload = ordered.map((r) => JSON.stringify(r)).join('\n') + '\n'
      await fsp.writeFile(this.file, payload, 'utf8') // fresh, in order
      const last = ordered[ordered.length - 1]!
      this.cursor = { head, pos: last._posEnd ?? 0, lastUuid: last.uuid, lastPos: last._pos }
      await this.writeSidecar()
    }).catch(() => { /* mirror is an optimisation: never break the session */ })
    return this.queue
  }

  /** The server says its file is a different one (compaction rewrote it). Start
   *  over rather than extend something that is no longer a prefix. */
  async reset(): Promise<void> {
    this.queue = this.queue.then(async () => {
      await this.close()
      this.cursor = null
      await fsp.rm(this.file, { force: true })
      await fsp.rm(this.sidecar, { force: true })
    }).catch(() => { /* best effort */ })
    return this.queue
  }

  async close(): Promise<void> {
    // Отложенный sidecar доносим при закрытии — курсор не должен пропасть.
    if (this.sidecarTimer) {
      clearTimeout(this.sidecarTimer)
      this.sidecarTimer = null
      await this.writeSidecar().catch(() => { /* best effort */ })
    }
    const s = this.stream
    this.stream = null
    if (!s) return
    await new Promise<void>((resolve) => { s.end(() => { resolve() }) })
  }

  private sidecarTimer: ReturnType<typeof setTimeout> | null = null

  /** Server offsets address the original JSONL, not JSON.stringify's mirror.
   * Locate their local byte position without parsing the entire disk cache. */
  private async localPosition(beforePos: number): Promise<number> {
    const fh = await fsp.open(this.file, 'r').catch(() => null)
    if (!fh) return 0
    try {
      let low = 0
      let high = (await fh.stat()).size
      const buf = Buffer.alloc(64 * 1024)
      while (low < high) {
        const mid = Math.floor((low + high) / 2)
        let rowStart = mid
        while (rowStart > low) {
          const from = Math.max(low, rowStart - buf.length)
          const read = await fh.read(buf, 0, rowStart - from, from)
          const nl = buf.subarray(0, read.bytesRead).lastIndexOf(10)
          rowStart = nl >= 0 ? from + nl + 1 : from
          if (nl >= 0) break
        }
        let at = rowStart
        let text = ''
        let found = false
        // Bound pathological records; the normal case takes one read.
        for (let n = 0; n < 256; n++) {
          const { bytesRead } = await fh.read(buf, 0, buf.length, at)
          if (!bytesRead) break
          const a = 0
          const nl = buf.subarray(0, bytesRead).indexOf(10, a)
          text += buf.toString('utf8', a, nl < 0 ? bytesRead : nl)
          if (nl >= 0) {
            const row = JSON.parse(text) as { _pos?: number }
            if (typeof row._pos === 'number' && row._pos < beforePos) low = at + nl + 1
            else high = rowStart
            found = true
            break
          }
          at += bytesRead
        }
        if (!found) high = mid
      }
      return low
    } catch { return 0 } finally { await fh.close() }
  }

  /** Дебаунс-запись sidecar (~1с): курсор двигается часто, диск — редко. */
  private scheduleSidecar(): void {
    if (this.sidecarTimer) return
    this.sidecarTimer = setTimeout(() => {
      this.sidecarTimer = null
      void this.writeSidecar().catch(() => { /* best effort */ })
    }, 1000)
  }

  private async writeSidecar(): Promise<void> {
    if (!this.cursor) return
    const st = await fsp.stat(this.file).catch(() => null)
    const body: Sidecar = { v: 3, ...this.cursor, ...(st ? { size: st.size } : {}) }
    // Write-then-rename: a torn sidecar would be read as a valid cursor and send
    // us asking for the wrong offset.
    const tmp = `${this.sidecar}.tmp`
    await fsp.writeFile(tmp, JSON.stringify(body), 'utf8')
    await fsp.rename(tmp, this.sidecar)
  }
}
