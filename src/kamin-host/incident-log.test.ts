import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import { createHostIncidentLogs } from "./incident-log"

const dirs: string[] = []
function tempDir(): string {
  const dir = mkdtempSync(join(tmpdir(), "kamin-incident-log-"))
  dirs.push(dir)
  return dir
}

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true })
})

describe("host incident logs", () => {
  it("keeps raw extension data out of the durable incident trail", () => {
    const dir = tempDir()
    const logs = createHostIncidentLogs(dir)
    logs.writeRaw("token=secret prompt=private\n")
    logs.writeExtensionExit(42, 1, null)
    logs.close()

    expect(readFileSync(join(dir, "host.log"), "utf8")).toContain("token=secret")
    const incident = readFileSync(join(dir, "incident.log"), "utf8")
    expect(incident).toContain('"event":"process-exit"')
    expect(incident).not.toContain("token=secret")
    expect(incident).not.toContain("prompt=private")
  })

  it("drops the prior raw run while preserving a prior sanitized trail", () => {
    const dir = tempDir()
    const first = createHostIncidentLogs(dir)
    first.writeRaw("previous raw")
    first.close()
    const second = createHostIncidentLogs(dir)
    second.writeRaw("current raw")
    second.close()

    expect(readFileSync(join(dir, "host.log"), "utf8")).toBe("current raw")
    expect(readFileSync(join(dir, "incident.log.1"), "utf8")).toContain('"event":"host-log-start"')
  })
})

function wholeIncidentRecords(file: string): Record<string, unknown>[] {
  const raw = readFileSync(file)
  const text = raw.toString("utf8")
  expect(Buffer.from(text, "utf8").equals(raw)).toBe(true)
  return text
    .split("\n")
    .filter((line) => line.length > 0)
    .map((line) => {
      expect(line.startsWith("[incident] ")).toBe(true)
      return JSON.parse(line.slice("[incident] ".length)) as Record<string, unknown>
    })
}

describe("incident records across rotation", () => {
  it("keeps every incident line a whole JSONL record in every generation", () => {
    const dir = tempDir()
    // 200 bytes holds one host-log-start record plus part of a process-exit
    // record: the byte-filling writer used to split the second one here.
    const logs = createHostIncidentLogs(dir, { incidentMaxBytes: 200 })
    for (let i = 0; i < 6; i += 1) logs.writeExtensionExit(1000 + i, i % 2, i % 3 === 0 ? "SIGTERM" : null)
    logs.close()

    const events: string[] = []
    for (const file of [join(dir, "incident.log"), join(dir, "incident.log.1"), join(dir, "incident.log.2"), join(dir, "incident.log.3")]) {
      if (!existsSync(file)) continue
      expect(statSync(file).size).toBeLessThanOrEqual(200)
      for (const rec of wholeIncidentRecords(file)) events.push(String(rec.event))
    }
    expect(events).toContain("process-exit")
    expect(events.every((event) => event === "host-log-start" || event === "process-exit")).toBe(true)
  })

  it("replaces an oversized record with a bounded marker instead of splitting or dropping silently", () => {
    const dir = tempDir()
    // The start record (~140 bytes) fits the cap; an exit record padded by a
    // long signal name does not and must surface only as a marker that names
    // it and its size, never as a split or silently missing line.
    const logs = createHostIncidentLogs(dir, { incidentMaxBytes: 300 })
    logs.writeExtensionExit(7, 1, `SIG${"X".repeat(400)}`)
    logs.close()

    const records = wholeIncidentRecords(join(dir, "incident.log"))
    expect(records.map((rec) => rec.event)).toContain("host-log-start")
    expect(records.map((rec) => rec.event)).not.toContain("process-exit")
    const markers = records.filter((rec) => rec.event === "record-dropped")
    expect(markers.map((rec) => rec.droppedEvent)).toEqual(["process-exit"])
    const [marker] = markers
    expect(typeof marker?.bytes).toBe("number")
    expect(Number(marker?.bytes)).toBeGreaterThan(300)
  })
})
