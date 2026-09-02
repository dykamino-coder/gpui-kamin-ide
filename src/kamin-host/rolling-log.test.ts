import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
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
