// ============================================================================
// Live tail of one transcript: one open descriptor, one read per pass.
//
// The CLI appends records; the watcher learns about it from fs.watch, a hook
// poke or the safety poll. Each wake-up used to cost stat + open + read +
// close + stat, five thread-pool round trips per chat per write, which under a
// burst of 200 streaming chats was a quarter of the server's main thread. A
// descriptor held for the life of the tail turns the pass into one positional
// read. The file's metadata is consulted only when that read comes back empty:
// that is the only moment a truncation or an in-place rewrite is observable
// at our offset. A replaced path (atomic rename) is reported by fs.watch as a
// `rename` event, on which the owner calls `close()`; the next pass reopens and
// compares the identity. Windows refuses to replace a file someone holds open
// (the CLI's atomic rename would fail), so there the descriptor lives for one
// pass only.
//
// Records are tagged with their exact byte span (`_pos`/`_posEnd`), the same
// tags a page or a top-up carries, so a client mirror advances on live records
// as well and a reattach resumes exactly where the client stopped.
// ============================================================================

import fsp, { type FileHandle } from 'fs/promises'
import type { JsonlEntry } from '../../shared/jsonl-types'

export interface TailPass {
  /** Parsed records, each tagged with `_pos`/`_posEnd`. */
  entries: JsonlEntry[]
  spans: Array<[number, number]>
  /** Boundary just past the last consumed line (skipped and malformed lines included). */
  endPos: number
  /** More complete records may follow (the batch cap was reached). */
  truncated: boolean
  /** Nothing new at `offset`; `stat` says whether the file shrank or was rewritten. */
  stat?: { size: number; mtimeMs: number; identity: string }
}

const MIN_READ = 64 * 1024
const HOLD_DESCRIPTOR = process.platform !== 'win32'

export class TranscriptTail {
  private fh: FileHandle | null = null
  private opening: Promise<FileHandle> | null = null
  private buf: Buffer = Buffer.allocUnsafe(MIN_READ)
  /** dev:ino of the open descriptor. */
  identity: string | null = null

  constructor(
    readonly filePath: string,
    private readonly skipMarker: string,
    private readonly batchBytes: number,
  ) {}

  private async handle(): Promise<FileHandle> {
    if (this.fh) return this.fh
    this.opening ??= (async () => {
      const fh = await fsp.open(this.filePath, 'r')
      const st = await fh.stat()
      this.identity = `${st.dev}:${st.ino}`
      this.fh = fh
      return fh
    })().finally(() => {
      this.opening = null
    })
    return this.opening
  }

  private async release(): Promise<void> {
    const fh = this.fh
    this.fh = null
    this.identity = null
    await fh?.close().catch(() => {})
  }

  /** Forget the descriptor (path replaced, file switched, watcher stopped). */
  close(): void {
    const fh = this.fh
    this.fh = null
    this.identity = null
    if (fh) void fh.close().catch(() => {})
  }

  /** Read complete records from `offset`, at most `batchBytes` of them (a single
   *  longer record is still read whole). A torn trailing line stays unconsumed. */
  async read(offset: number): Promise<TailPass> {
    try {
      return await this.readOnce(offset)
    } finally {
      if (!HOLD_DESCRIPTOR) await this.release()
    }
  }

  private async readOnce(offset: number): Promise<TailPass> {
    const fh = await this.handle()
    let filled = 0
    let eof = false
    // One read fills the buffer. Only a record longer than the whole buffer
    // makes it grow (past the batch cap if it must: a record is read whole).
    for (;;) {
      const want = this.buf.length - filled
      const { bytesRead } = await fh.read(this.buf, filled, want, offset + filled)
      filled += bytesRead
      if (bytesRead < want) eof = true
      if (eof || this.buf.lastIndexOf(0x0a, filled - 1) !== -1) break
      const next = Buffer.allocUnsafe(this.buf.length * 2)
      this.buf.copy(next, 0, 0, filled)
      this.buf = next
    }
    if (filled === 0) {
      // Metadata of the PATH: a shrink, an in-place rewrite or a replacement
      // that fs.watch did not report is only observable here.
      const st = await fsp.stat(this.filePath)
      return {
        entries: [],
        spans: [],
        endPos: offset,
        truncated: false,
        stat: { size: st.size, mtimeMs: st.mtimeMs, identity: `${st.dev}:${st.ino}` },
      }
    }
    const pass = this.parse(offset, filled)
    // The buffer filled before the file ended: more records wait behind it.
    pass.truncated = !eof
    // Size the buffer to the traffic: grow while passes fill it (up to the
    // batch cap), drop back once a burst or one huge record has passed.
    if (filled === this.buf.length && this.buf.length < this.batchBytes)
      this.buf = Buffer.allocUnsafe(Math.min(this.batchBytes, this.buf.length * 2))
    else if (this.buf.length > MIN_READ && filled < MIN_READ / 2) this.buf = Buffer.allocUnsafe(MIN_READ)
    return pass
  }

  private parse(offset: number, filled: number): TailPass {
    const entries: JsonlEntry[] = []
    const spans: Array<[number, number]> = []
    let endPos = offset
    let from = 0
    while (from < filled) {
      const nl = this.buf.indexOf(0x0a, from)
      if (nl === -1 || nl >= filled) break // torn tail: the next pass re-reads it whole
      const start = offset + from
      const end = offset + nl + 1
      let text = this.buf.toString('utf8', from, nl)
      if (text.endsWith('\r')) text = text.slice(0, -1)
      from = nl + 1
      endPos = end
      if (!text.trim() || text.includes(this.skipMarker)) continue
      let parsed: JsonlEntry
      try {
        parsed = JSON.parse(text) as JsonlEntry
      } catch {
        continue // terminated but malformed: it will never complete
      }
      const tagged = parsed as JsonlEntry & { _pos?: number; _posEnd?: number }
      tagged._pos = start
      tagged._posEnd = end
      entries.push(parsed)
      spans.push([start, end])
    }
    return { entries, spans, endPos, truncated: false }
  }
}
