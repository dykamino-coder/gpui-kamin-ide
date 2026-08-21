import { mkdtempSync, readFileSync, rmSync } from "node:fs"
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
