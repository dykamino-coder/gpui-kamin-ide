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
