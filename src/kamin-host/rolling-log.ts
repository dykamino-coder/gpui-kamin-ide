// Bounded synchronous writer for the extension-host incident trail.
//
// Child stdout/stderr arrives in already-sized pipe chunks. Synchronous writes
// keep rotation deterministic on Windows: closing the descriptor before rename
// avoids racing a still-flushing WriteStream against `MoveFileEx` semantics.

import {
  closeSync,
  existsSync,
  openSync,
  renameSync,
  statSync,
  unlinkSync,
  writeSync,
} from "node:fs"

export interface RollingLogOptions {
  maxBytes: number
  backups: number
  rotateOnOpen?: boolean
}

function removeIfPresent(path: string): void {
  try { if (existsSync(path)) unlinkSync(path) } catch { /* best-effort log */ }
}

/** Shift `path.N` backups and move the current file to `path.1`. */
export function rotateLogFiles(path: string, backups: number): boolean {
  if (backups <= 0) {
    removeIfPresent(path)
    return !existsSync(path)
  }
  removeIfPresent(`${path}.${String(backups)}`)
  for (let i = backups - 1; i >= 1; i -= 1) {
    const from = `${path}.${String(i)}`
    if (!existsSync(from)) continue
    const to = `${path}.${String(i + 1)}`
    removeIfPresent(to)
    try { renameSync(from, to) } catch { /* best-effort log */ }
  }
  if (existsSync(path)) {
    removeIfPresent(`${path}.1`)
    try { renameSync(path, `${path}.1`) } catch { return false }
  }
  return true
}

export class RollingLogWriter {
  private fd: number | null = null
  private bytes = 0
  private permanentlyClosed = false

  constructor(
    readonly path: string,
    private readonly options: RollingLogOptions,
  ) {
    if (!Number.isSafeInteger(options.maxBytes) || options.maxBytes <= 0) {
      throw new Error("rolling log maxBytes must be a positive safe integer")
    }
    if (!Number.isSafeInteger(options.backups) || options.backups < 0) {
      throw new Error("rolling log backups must be a non-negative safe integer")
    }
    if (options.rotateOnOpen && existsSync(path) && statSync(path).size > 0) {
      rotateLogFiles(path, options.backups)
    }
    this.open()
  }

  /**
   * Append one structured record atomically with respect to rotation.
   *
   * `write()` fills the remaining bytes of the current generation before it
   * rotates — right for a raw stream, wrong for `[incident] <json>` lines:
   * a rotation boundary inside the record left its head in `.1` and its tail
   * in the new file, and neither fragment parsed as JSONL. Here a record that
   * fits an empty generation but not the current remainder rotates first and
   * is then written whole. A record larger than `maxBytes` is refused (returns
   * `false`) instead of weakening the retention cap; the caller decides what
   * bounded marker to emit instead.
   */
  writeRecord(record: string | Buffer): boolean {
    if (this.permanentlyClosed) return false
    const data = Buffer.isBuffer(record) ? record : Buffer.from(record, "utf8")
    if (data.length === 0) return true
    if (data.length > this.options.maxBytes) return false
    if (this.fd === null) this.open()
    if (this.bytes > 0 && this.bytes + data.length > this.options.maxBytes) this.rotate()
    // Read through a method: `rotate()` may have closed the descriptor, and a
    // plain property check here would be narrowed away by the compiler.
    const fd = this.descriptor()
    if (fd === null) return false
    let offset = 0
    while (offset < data.length) {
      try {
        const written = writeSync(fd, data, offset, data.length - offset)
        if (written <= 0) return false
        this.bytes += written
        offset += written
      } catch {
        this.closeDescriptor()
        return false
      }
    }
    return true
  }

  write(chunk: string | Buffer): void {
    if (this.permanentlyClosed) return
    const data = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, "utf8")
    let offset = 0
    while (offset < data.length) {
      if (this.bytes >= this.options.maxBytes) this.rotate()
      const remaining = this.options.maxBytes - this.bytes
      const size = Math.min(remaining, data.length - offset)
      if (size <= 0) break
      try {
        if (this.fd === null) this.open()
        if (this.fd === null) return
        const written = writeSync(this.fd, data, offset, size)
        if (written <= 0) return
        this.bytes += written
        offset += written
      } catch {
        this.closeDescriptor()
        return
      }
    }
  }

  close(): void {
    this.permanentlyClosed = true
    this.closeDescriptor()
  }

  private descriptor(): number | null {
    return this.fd
  }

  private closeDescriptor(): void {
    if (this.fd === null) return
    try { closeSync(this.fd) } catch { /* best-effort log */ }
    this.fd = null
  }

  private open(): void {
    if (this.permanentlyClosed) return
    try {
      this.fd = openSync(this.path, "a")
      try {
        this.bytes = statSync(this.path).size
      } catch {
        this.closeDescriptor()
        this.bytes = 0
        return
      }
      if (this.bytes >= this.options.maxBytes) this.rotate()
    } catch {
      this.fd = null
      this.bytes = 0
    }
  }

  private rotate(): void {
    this.closeDescriptor()
    const rotated = rotateLogFiles(this.path, this.options.backups)
    this.bytes = 0
    try {
      this.fd = openSync(this.path, rotated ? "a" : "w")
      this.bytes = statSync(this.path).size
    } catch {
      this.closeDescriptor()
      this.bytes = 0
    }
  }
}
