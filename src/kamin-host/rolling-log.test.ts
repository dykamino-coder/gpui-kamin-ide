import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import { RollingLogWriter, rotateLogFiles } from "./rolling-log"

const dirs: string[] = []
function tempPath(): string {
  const dir = mkdtempSync(join(tmpdir(), "kamin-rolling-log-"))
  dirs.push(dir)
  return join(dir, "host.log")
}

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true })
})

describe("RollingLogWriter", () => {
  it("preserves the previous boot and caps every retained generation", () => {
    const path = tempPath()
    writeFileSync(path, "previous")
    const log = new RollingLogWriter(path, { maxBytes: 8, backups: 2, rotateOnOpen: true })
    log.write("abcdefghijkl")
    log.close()

    expect(readFileSync(`${path}.2`, "utf8")).toBe("previous")
    expect(readFileSync(`${path}.1`, "utf8")).toBe("abcdefgh")
    expect(readFileSync(path, "utf8")).toBe("ijkl")
    for (const file of [path, `${path}.1`, `${path}.2`]) {
      expect(Buffer.byteLength(readFileSync(file))).toBeLessThanOrEqual(8)
    }
  })

  it("keeps only the configured number of backups", () => {
    const path = tempPath()
    writeFileSync(path, "zero")
    writeFileSync(`${path}.1`, "one")
    writeFileSync(`${path}.2`, "two")
    expect(rotateLogFiles(path, 2)).toBe(true)

    expect(readFileSync(`${path}.1`, "utf8")).toBe("zero")
    expect(readFileSync(`${path}.2`, "utf8")).toBe("one")
  })

  it("does not reopen after permanent close", () => {
    const path = tempPath()
    const log = new RollingLogWriter(path, { maxBytes: 8, backups: 1 })
    log.write("before")
    log.close()
    log.write("after")

    expect(readFileSync(path, "utf8")).toBe("before")
  })
})

/** Every non-empty line of a retained generation must be one whole record:
 *  full prefix, valid JSON, valid UTF-8 (a boundary inside a multibyte
 *  character would decode to U+FFFD). */
function expectWholeRecords(file: string, prefix: string): string[] {
  const raw = readFileSync(file)
  const text = raw.toString("utf8")
  expect(Buffer.from(text, "utf8").equals(raw)).toBe(true)
  expect(text).not.toContain("\uFFFD")
  const lines = text.split("\n").filter((line) => line.length > 0)
  for (const line of lines) {
    expect(line.startsWith(prefix)).toBe(true)
    expect(() => { JSON.parse(line.slice(prefix.length)) }).not.toThrow()
  }
  return lines
}

describe("RollingLogWriter.writeRecord", () => {
  const prefix = "[incident] "
  const record = (event: string, extra: Record<string, unknown> = {}): string =>
    `${prefix}${JSON.stringify({ event, ...extra })}\n`

  it("rotates before a record that does not fit the remainder instead of splitting it", () => {
    const path = tempPath()
    const first = record("a")
    const second = record("bb")
    // Cap = first + half of second: the old byte-filling write() would split
    // `second` across the boundary; the record path must rotate first.
    const maxBytes = Buffer.byteLength(first) + Math.floor(Buffer.byteLength(second) / 2)
    const log = new RollingLogWriter(path, { maxBytes, backups: 2 })
    expect(log.writeRecord(first)).toBe(true)
    expect(log.writeRecord(second)).toBe(true)
    log.close()

    expect(expectWholeRecords(`${path}.1`, prefix)).toEqual([first.trimEnd()])
    expect(expectWholeRecords(path, prefix)).toEqual([second.trimEnd()])
    for (const file of [path, `${path}.1`]) {
      expect(Buffer.byteLength(readFileSync(file))).toBeLessThanOrEqual(maxBytes)
    }
  })

  it("never places the rotation boundary inside a multibyte character", () => {
    const path = tempPath()
    // Cyrillic payload: every character is 2 bytes, so a byte boundary that
    // is not record-aligned is very likely to land inside a code point.
    const records = Array.from({ length: 12 }, (_, i) => record("вызов", { i, текст: "проверка записи" }))
    const maxBytes = Buffer.byteLength(records[0] ?? "") + 7
    const log = new RollingLogWriter(path, { maxBytes, backups: 12 })
    for (const line of records) expect(log.writeRecord(line)).toBe(true)
    log.close()

    const kept: string[] = []
    for (const file of [path, ...Array.from({ length: 12 }, (_, i) => `${path}.${String(i + 1)}`)]) {
      if (!existsSync(file)) continue
      kept.push(...expectWholeRecords(file, prefix))
    }
    // Every record survived exactly once, each in some generation, unsplit.
    expect(kept.sort()).toEqual(records.map((line) => line.trimEnd()).sort())
  })

  it("keeps exactly-fitting records in one generation and a fresh file cannot be over-filled", () => {
    const path = tempPath()
    const line = record("fit")
    const log = new RollingLogWriter(path, { maxBytes: Buffer.byteLength(line) * 2, backups: 1 })
    expect(log.writeRecord(line)).toBe(true)
    expect(log.writeRecord(line)).toBe(true)
    expect(log.writeRecord(line)).toBe(true)
    log.close()

    expect(expectWholeRecords(`${path}.1`, prefix)).toHaveLength(2)
    expect(expectWholeRecords(path, prefix)).toHaveLength(1)
  })

  it("refuses a record larger than the cap without touching the current generation", () => {
    const path = tempPath()
    const log = new RollingLogWriter(path, { maxBytes: 32, backups: 1 })
    expect(log.writeRecord(record("ok"))).toBe(true)
    const before = readFileSync(path, "utf8")
    expect(log.writeRecord(record("x".repeat(64)))).toBe(false)
    log.close()

    expect(readFileSync(path, "utf8")).toBe(before)
    expect(existsSync(`${path}.1`)).toBe(false)
  })

  it("leaves the raw byte-filling write() unchanged", () => {
    const path = tempPath()
    const log = new RollingLogWriter(path, { maxBytes: 8, backups: 1 })
    log.write("abcdefghij")
    log.close()

    expect(readFileSync(`${path}.1`, "utf8")).toBe("abcdefgh")
    expect(readFileSync(path, "utf8")).toBe("ij")
  })
})
