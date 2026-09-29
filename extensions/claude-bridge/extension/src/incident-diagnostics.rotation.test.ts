import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { afterEach, describe, expect, it, vi } from "vitest"

/** INC-2026-0026: запись инцидента обязана целиком лежать в ОДНОМ поколении.
 *
 *  Прежний эмиттер шёл через `RollingLogWriter.write`, который дозаполняет
 *  остаток текущего поколения и вращает журнал ПОСЕРЕДИНЕ строки: голова
 *  оставалась в `.1`, хвост попадал в новый файл, и ни один обломок не
 *  разбирался как JSONL. Поэтому проверка идёт через настоящий модуль
 *  расширения и настоящую ротацию на боевом пороге, а не через помощника. */

/** Многобайтовая версия: разрыв внутри UTF-8 последовательности JSON-разбор
 *  переживает не всегда, а вот значение поля портит всегда. */
const APP_VERSION = "версия-1.0.60-Ω"
const LOG_CAP_BYTES = 1024 * 1024
const ROLES = ["chat", "tools", "customize"] as const

let dirs: string[] = []

afterEach(() => {
  for (const dir of dirs) rmSync(dir, { recursive: true, force: true })
  dirs = []
  delete process.env.KAMIN_APP_VERSION
})

function tempDir(): string {
  const dir = mkdtempSync(join(tmpdir(), "inc-0026-"))
  dirs.push(dir)
  return dir
}

async function freshModule(): Promise<typeof import("./incident-diagnostics")> {
  vi.resetModules()
  return import("./incident-diagnostics")
}

/** Заполнить журнал до ротации: записи боевого размера, пока не перевалим кап. */
async function fillPastRotation(dir: string): Promise<number> {
  const mod = await freshModule()
  mod.installIncidentDiagnostics(dir, () => undefined)
  let emitted = 0
  let now = 0
  while (emitted * 280 < LOG_CAP_BYTES * 1.5) {
    now += 60_000
    for (const role of ROLES) {
      mod.recordRendererSample({
        role,
        heapMB: 123.4,
        retainedTabs: 12,
        retainedEntries: 3456,
        activeEntries: 789,
        storeWindow: 2000,
        scrollUpMax: 500,
        windowState: "within-configured-window",
      }, now)
      emitted += 1
    }
  }
  return emitted
}

/** Все удержанные поколения журнала — текущее и `.1`…`.3`. */
function generations(dir: string): { name: string; text: string }[] {
  return readdirSync(dir)
    .filter((name) => name.startsWith("incident.log"))
    .sort()
    .map((name) => ({ name, text: readFileSync(join(dir, name), "utf8") }))
}

describe("INC-2026-0026: запись инцидента не рвётся ротацией", () => {
  it("каждое удержанное поколение разбирается построчно", async () => {
    process.env.KAMIN_APP_VERSION = APP_VERSION
    const dir = tempDir()

    await fillPastRotation(dir)

    const files = generations(dir)
    expect(files.length).toBeGreaterThan(1) // ротация действительно случилась
    for (const file of files) {
      const lines = file.text.split("\n").filter((line) => line.length > 0)
      expect(lines.length).toBeGreaterThan(0)
      for (const line of lines) {
        expect(line.startsWith("[incident] "), `${file.name}: обломок строки`).toBe(true)
        expect(() => JSON.parse(line.slice("[incident] ".length))).not.toThrow()
      }
    }
  })

  it("многобайтовое поле не рвётся на границе поколений", async () => {
    process.env.KAMIN_APP_VERSION = APP_VERSION
    const dir = tempDir()

    await fillPastRotation(dir)

    for (const file of generations(dir)) {
      for (const line of file.text.split("\n").filter((l) => l.length > 0)) {
        const record = JSON.parse(line.slice("[incident] ".length)) as { appVersion?: unknown }
        expect(record.appVersion, `${file.name}: версия побилась`).toBe(APP_VERSION)
      }
    }
  })

  it("перезапуск с ротацией при открытии оставляет прежние поколения целыми", async () => {
    process.env.KAMIN_APP_VERSION = APP_VERSION
    const dir = tempDir()
    await fillPastRotation(dir)

    // Второй запуск процесса над тем же каталогом: `rotateOnOpen` сдвигает
    // поколения, и уже лежащие записи обязаны пережить сдвиг без изменений.
    const before = generations(dir).map((f) => f.text).join("")
    await fillPastRotation(dir)

    const after = generations(dir).map((f) => f.text).join("")
    expect(after.length).toBeGreaterThan(0)
    for (const line of after.split("\n").filter((l) => l.length > 0)) {
      expect(() => JSON.parse(line.slice("[incident] ".length))).not.toThrow()
    }
    expect(before.length).toBeGreaterThan(0)
  })
})
