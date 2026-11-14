// Copies a segment's native transcript into the conversation's unified log
// (unified-log.ts) as the engine writes it.
//
// The native transcript lives in the segment's own slug directory: the Claude
// CLI's JSONL, or the normalized transcript Bridge writes for a codex thread.
// The copy position (file + byte offset) is reported after every copy and kept
// in the manifest, so a restarted server continues where it stopped instead of
// duplicating or skipping entries. A Claude compaction continues the session in
// a NEW file of the same directory: once the current file is copied to its end,
// the pump moves on to the newer file from its start.

import fs from 'node:fs'
import path from 'node:path'
import type { JsonlEntry } from '../../shared/jsonl-types'
import type { UnifiedLog } from './unified-log'

const POLL_MS = 250

export interface PumpPosition {
  file: string | null
  pos: number
}

export interface SegmentPumpOptions {
  /** Slug directory holding the segment's native transcript(s). */
  dir: string
  log: UnifiedLog
  from: PumpPosition
  /** Native files modified before this are not the segment's (ms). */
  since: number
  onProgress?: (position: PumpPosition) => void
  /** Native session id, from the transcript file name. */
  onNativeId?: (id: string) => void
  /** After the final copy of stop(). */
  onStop?: (position: PumpPosition) => void
  pollMs?: number
}

function newestTranscript(dir: string, since: number, after?: { file: string; mtime: number }): string | null {
  let best: { file: string; mtime: number } | null = null
  let names: string[]
  try {
    names = fs.readdirSync(dir)
  } catch {
    return null
  }
  for (const name of names) {
    if (!name.endsWith('.jsonl')) continue
    const file = path.join(dir, name)
    let st: fs.Stats
    try {
      st = fs.statSync(file)
    } catch {
      continue
    }
    if (!st.isFile() || st.mtimeMs < since) continue
    if (after && (file === after.file || st.mtimeMs <= after.mtime)) continue
    if (!best || st.mtimeMs > best.mtime) best = { file, mtime: st.mtimeMs }
  }
  return best?.file ?? null
}

export class SegmentPump {
  private position: PumpPosition
  private timer: ReturnType<typeof setInterval> | null = null
  private copying = false

  constructor(private readonly options: SegmentPumpOptions) {
    this.position = { ...options.from }
  }

  get at(): PumpPosition {
    return { ...this.position }
  }

  start(): void {
    if (this.timer) return
    this.flush()
    this.timer = setInterval(() => this.flush(), this.options.pollMs ?? POLL_MS)
    this.timer.unref?.()
  }

  stop(): void {
    if (this.timer) clearInterval(this.timer)
    this.timer = null
    this.flush()
    this.options.onStop?.(this.at)
  }

  /** Copy everything complete that the native transcript holds now. */
  flush(): void {
    if (this.copying) return
    this.copying = true
    try {
      for (let guard = 0; guard < 8; guard++) {
        if (!this.position.file) {
          const first = newestTranscript(this.options.dir, this.options.since)
          if (!first) return
          this.adopt(first)
        }
        this.copyTail()
        // Compaction: a newer file continues the session.
        const file = this.position.file!
        let mtime = 0
        try {
          mtime = fs.statSync(file).mtimeMs
        } catch {
          /* gone: look for its successor */
        }
        const next = newestTranscript(this.options.dir, this.options.since, { file, mtime })
        if (!next) return
        this.adopt(next)
      }
    } finally {
      this.copying = false
    }
  }

  private adopt(file: string): void {
    this.position = { file, pos: 0 }
    this.options.onNativeId?.(path.basename(file, '.jsonl'))
    this.options.onProgress?.(this.at)
  }

  private copyTail(): void {
    const file = this.position.file!
    let fd: number
    try {
      fd = fs.openSync(file, 'r')
    } catch {
      return
    }
    try {
      const size = fs.fstatSync(fd).size
      if (size <= this.position.pos) return
      const buf = Buffer.alloc(size - this.position.pos)
      fs.readSync(fd, buf, 0, buf.length, this.position.pos)
      const end = buf.lastIndexOf(0x0a)
      if (end < 0) return // no complete line yet
      const entries: JsonlEntry[] = []
      for (const line of buf.subarray(0, end).toString('utf8').split('\n')) {
        if (!line.trim()) continue
        try {
          entries.push(JSON.parse(line) as JsonlEntry)
        } catch {
          /* a torn line is skipped, like the transcript watcher does */
        }
      }
      this.options.log.append(entries)
      this.position = { file, pos: this.position.pos + end + 1 }
      this.options.onProgress?.(this.at)
    } finally {
      fs.closeSync(fd)
    }
  }
}
