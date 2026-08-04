// The index cache moved from JSON to tab-separated lines: on a real 248 660-file
// root JSON was 71.8MB / 521ms, the lines are 28.8MB / 180ms. `abs` is no longer
// stored — it is `root + rel` — so a load must reconstruct it, and a subtly wrong
// reconstruction would silently hand out paths that resolve nowhere.
import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, it, expect, afterAll } from "vitest"
import { initIndexCacheDir, __testing } from "./file-index.js"

const dirs: string[] = []
async function freshCacheRoot(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "kamin-idx-"))
  dirs.push(dir)
  initIndexCacheDir(dir)
  return dir
}
afterAll(async () => {
  for (const d of dirs) await rm(d, { recursive: true, force: true })
})

const ROOT = join(tmpdir(), "some-project")
const FILES = [
  { rel: "src/main.ts", abs: join(ROOT, "src/main.ts"), size: 120, mtimeMs: 1_700_000_000_000 },
  { rel: "docs/a b/c.md", abs: join(ROOT, "docs/a b/c.md"), size: 0, mtimeMs: 1 },
  // Non-ASCII really occurs in this index — the real one holds Cyrillic names.
  { rel: "прочее/файл.rar", abs: join(ROOT, "прочее/файл.rar"), size: 3_045_178_367, mtimeMs: 42 },
]

describe("file index cache", () => {
  it("round-trips entries, rebuilding abs from root + rel", async () => {
    await freshCacheRoot()
    await __testing.saveCache(ROOT, FILES)
    const loaded = await __testing.loadCache(ROOT)
    expect(loaded).toEqual(FILES)
  })

  it("keeps sizes past 2GB intact", async () => {
    await freshCacheRoot()
    await __testing.saveCache(ROOT, FILES)
    const loaded = await __testing.loadCache(ROOT)
    expect(loaded?.find((f) => f.rel === "прочее/файл.rar")?.size).toBe(3_045_178_367)
  })

  it("refuses a cache written for a DIFFERENT root", async () => {
    await freshCacheRoot()
    await __testing.saveCache(ROOT, FILES)
    expect(await __testing.loadCache(join(tmpdir(), "other-project"))).toBeNull()
  })

  it("returns null instead of throwing when there is no cache", async () => {
    await freshCacheRoot()
    expect(await __testing.loadCache(ROOT)).toBeNull()
  })

  it("writes the compact line form, not JSON", async () => {
    const dir = await freshCacheRoot()
    await __testing.saveCache(ROOT, FILES)
    void dir
    const written = await readFile(join(__testing.cacheDirFor(ROOT), __testing.CACHE_FILE), "utf8")
    expect(written.startsWith("v2\t")).toBe(true)
    expect(written).not.toContain("{")
    // One header line + one line per entry, and no `abs` duplicated into it.
    expect(written.split("\n")).toHaveLength(FILES.length + 1)
    expect(written).not.toContain(`${ROOT  }\\src`)
  })
})
