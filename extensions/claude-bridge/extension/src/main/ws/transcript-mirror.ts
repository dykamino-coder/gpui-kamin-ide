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
}

// v2: зеркала, записанные ДО фикса out-of-order курсора (#82), могут нести
// завышенный pos и дырявый файл НАВСЕГДА — синк с такого курсора молча теряет
// хвост при каждом рестарте. Бамп версии отвергает все v1-сайдкары: одна цена —
// один полный реплей на сессию, зеркало пересоздаётся честным rewriteInOrder.
interface Sidecar extends MirrorCursor {
  v: 2
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
      if (parsed.v !== 2 || typeof parsed.pos !== 'number' || !parsed.head) return null
      // A sidecar claiming more than the file holds means we died mid-write;
      // trusting it would ask the server to resume past records we never stored.
      const stat = await fsp.stat(this.file)
      if (stat.size === 0) return null
      // Local-size check (C9): mirror shorter than at sidecar write = torn
      // tail → the cursor lies about what we hold; full pull instead of a
      // permanent history hole. `>=` is fine — appends after the debounced
      // sidecar write only make the file longer.
      if (typeof parsed.size === 'number' && stat.size < parsed.size) return null
      this.cursor = { head: parsed.head, pos: parsed.pos, lastUuid: parsed.lastUuid }
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
   *  Reads only a BYTE WINDOW ending at `beforePos`, never the whole file — `_pos`
   *  IS the byte offset, so an older page is a bounded seek+read in the host
   *  process, not a full parse that would churn the host heap on every scroll. A
   *  window too small to hold `count` records just returns fewer; the caller asks
   *  again as the user keeps scrolling. */
  async readRange(beforePos: number, count: number): Promise<unknown[]> {
    // Одна запись КРУПНЕЕ окна (жирный tool_result: зеркало хранит полные
    // записи, trimHeavyToolResults живёт только в webview-сторе) давала
    // пустую страницу — рендерер трактовал её как «начало файла» и навсегда
    // выключал скролл-ап (аудит #70 C10). Пустой результат при beforePos
    // больше окна → повторить с удвоенным окном, до 3 попыток.
    let window = Math.max(256 * 1024, count * 4096)
    for (let attempt = 0; attempt < 3; attempt++) {
      const out = await this.readRangeWindow(beforePos, count, window)
      if (out.length > 0 || beforePos <= window) return out
      window *= 4
    }
    return []
  }

  private async readRangeWindow(beforePos: number, count: number, windowSize: number): Promise<unknown[]> {
    if (beforePos <= 0 || count <= 0) return []
    // Sized to comfortably hold `count` records; the first (partial) line of a
    // mid-file read is dropped below.
    const window = Math.min(beforePos, windowSize)
    const start = beforePos - window
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
    // Chronological by `timestamp`, NOT file byte order. The mirror is written in
    // `_pos` (server byte-offset) order, and `_pos` RESETS to 0 on every resume
    // file — so a resumed session's pre-resume (ancient) turns sink to the tail of
    // the mirror file and a naive "last lines" read paints them as the current
    // chat. `timestamp` is monotonic across resumes; sort the window by it and
    // return the NEWEST `maxRecords`, so the fast tail-paint is the real end.
    recs.sort((a, b) => (a.timestamp ?? '').localeCompare(b.timestamp ?? ''))
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
    // Chronological by `timestamp` (tie-break `_pos`), NOT raw `_pos`. `_pos` is a
    // per-FILE byte offset that resets on every resume, so ordering a resumed
    // session's history by it interleaves the files and sinks old turns to the
    // tail (→ readTail paints ancient content). `timestamp` is monotonic across
    // resumes. For a single-file session ts-order == _pos-order, so this is a
    // no-op there and readRange (still byte-seek) is unaffected.
    const tsOf = (r: unknown): string => (r as { timestamp?: string }).timestamp ?? ''
    const ordered = [...byUuid.values(), ...noUuid].sort(
      (a, b) => tsOf(a).localeCompare(tsOf(b)) || (a._pos ?? 0) - (b._pos ?? 0),
    )
    if (ordered.length === 0) return
    this.queue = this.queue.then(async () => {
      // NEVER shrink an existing mirror. A session lived from the start builds a
      // full, UNCAPPED copy via append(), while `records` here is the ~20k-capped
      // in-process cache. This fires on every replay-complete (reconnect, reattach,
      // resume) — overwriting then would truncate history the mirror already holds
      // and the store can still scroll to: silent, permanent data loss. So only
      // populate a mirror that is EMPTY — precisely the resumed-session case where
      // the streamed history was never appended in the first place.
      try { const st = await fsp.stat(this.file); if (st.size > 0) return } catch { /* absent → write it */ }
      await this.close() // drop any append stream — we're initialising the file
      const payload = ordered.map((r) => JSON.stringify(r)).join('\n') + '\n'
      await fsp.writeFile(this.file, payload, 'utf8') // fresh, in order
      const last = ordered[ordered.length - 1]!
      this.cursor = { head, pos: last._posEnd ?? 0, lastUuid: last.uuid }
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
    const body: Sidecar = { v: 2, ...this.cursor, ...(st ? { size: st.size } : {}) }
    // Write-then-rename: a torn sidecar would be read as a valid cursor and send
    // us asking for the wrong offset.
    const tmp = `${this.sidecar}.tmp`
    await fsp.writeFile(tmp, JSON.stringify(body), 'utf8')
    await fsp.rename(tmp, this.sidecar)
  }
}
