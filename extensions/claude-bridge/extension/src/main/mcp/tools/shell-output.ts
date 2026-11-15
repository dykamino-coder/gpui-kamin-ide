// Output of one stream of a foreground command. Up to INLINE_BYTES is returned
// as is. Beyond that the whole stream goes to a file on this machine, where the
// model's Read and Grep can open it, and the result carries the head, the tail
// and the file path. Native Claude Code also keeps the full output in a file;
// returning only a capped head lost the end of long logs (INC-2026-0121).
import { randomBytes } from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { StringDecoder } from 'node:string_decoder'

export const INLINE_BYTES = 100 * 1024
export const HEAD_BYTES = 40 * 1024
export const TAIL_BYTES = 40 * 1024
/** A runaway command cannot fill the disk: the file stops growing here. */
export const FILE_CAP_BYTES = 256 * 1024 * 1024
const STALE_MS = 24 * 3600_000
let swept = false

export function outputDirectory(): string {
  return path.join(os.tmpdir(), 'kamin-bridge-output')
}

function sweepStale(directory: string): void {
  if (swept) return
  swept = true
  try {
    const now = Date.now()
    for (const name of fs.readdirSync(directory)) {
      const file = path.join(directory, name)
      try {
        if (now - fs.statSync(file).mtimeMs > STALE_MS) fs.unlinkSync(file)
      } catch {
        /* in use or already gone */
      }
    }
  } catch {
    /* best effort */
  }
}

function formatSize(bytes: number): string {
  return bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.round(bytes / 1024)} KB`
}

/** Text of `head` without a trailing partial UTF-8 character. */
function decodeHead(head: Buffer): string {
  return new StringDecoder('utf8').write(head)
}

/** Text of `tail` without a leading partial UTF-8 character. */
function decodeTail(tail: Buffer): string {
  let start = 0
  while (start < tail.length && start < 4 && (tail[start]! & 0xc0) === 0x80) start++
  return tail.subarray(start).toString('utf8')
}

export class StreamCapture {
  private chunks: Buffer[] = []
  private total = 0
  private fd: number | null = null
  private file: string | null = null
  private written = 0
  private saveError: string | null = null
  private head: Buffer | null = null
  private tail: Buffer[] = []
  private tailBytes = 0

  constructor(
    private readonly label: string,
    private readonly directory: string = outputDirectory(),
  ) {}

  push(chunk: Buffer): void {
    this.total += chunk.length
    if (this.head === null) {
      this.chunks.push(chunk)
      if (this.total > INLINE_BYTES) this.overflow()
      return
    }
    this.save(chunk)
    this.keepTail(chunk)
  }

  get bytes(): number {
    return this.total
  }

  private overflow(): void {
    const all = Buffer.concat(this.chunks)
    this.chunks = []
    this.head = all.subarray(0, HEAD_BYTES)
    try {
      fs.mkdirSync(this.directory, { recursive: true, mode: 0o700 })
      sweepStale(this.directory)
      this.file = path.join(this.directory, `${this.label}-${process.pid}-${randomBytes(6).toString('hex')}.txt`)
      this.fd = fs.openSync(this.file, 'wx', 0o600)
    } catch (err) {
      this.saveError = err instanceof Error ? err.message : String(err)
      this.file = null
    }
    this.save(all)
    this.keepTail(all)
  }

  private save(chunk: Buffer): void {
    if (this.fd === null) return
    const room = FILE_CAP_BYTES - this.written
    if (room <= 0) return
    const part = chunk.length > room ? chunk.subarray(0, room) : chunk
    try {
      fs.writeSync(this.fd, part)
      this.written += part.length
    } catch (err) {
      this.saveError = err instanceof Error ? err.message : String(err)
      this.close()
    }
  }

  private keepTail(chunk: Buffer): void {
    this.tail.push(chunk)
    this.tailBytes += chunk.length
    while (this.tail.length > 1 && this.tailBytes - this.tail[0]!.length >= TAIL_BYTES) {
      this.tailBytes -= this.tail.shift()!.length
    }
  }

  private close(): void {
    if (this.fd === null) return
    try {
      fs.closeSync(this.fd)
    } catch {
      /* already closed */
    }
    this.fd = null
  }

  /** The text returned to the model. Closes the file. */
  finish(): string {
    if (this.head === null) return Buffer.concat(this.chunks).toString('utf8')
    this.close()
    const tailAll = Buffer.concat(this.tail)
    const tail = tailAll.subarray(Math.max(0, tailAll.length - TAIL_BYTES))
    const omitted = this.total - this.head.length - tail.length
    const where = this.file
      ? this.written < this.total
        ? `The first ${formatSize(this.written)} were saved to ${this.file} (the file stops at ${formatSize(FILE_CAP_BYTES)}).`
        : `Full output saved to ${this.file}; read it with Read (offset/limit) or search it with Grep.`
      : `The full output could not be saved (${this.saveError ?? 'unknown error'}).`
    return (
      `${decodeHead(this.head)}\n\n... [${formatSize(omitted)} omitted] ...\n\n${decodeTail(tail)}` +
      `\n[Output too large: ${formatSize(this.total)}. Showing the first ${formatSize(HEAD_BYTES)} and the last ${formatSize(TAIL_BYTES)}. ${where}]`
    )
  }
}
