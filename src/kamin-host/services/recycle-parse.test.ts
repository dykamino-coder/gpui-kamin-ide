// Recycle Bin $I metadata parsing (the original-path field that drives
// restoreFromTrash). Covers the Win10 v2 layout, legacy v1, and short buffers.
import { describe, expect, it } from "vitest"
import { parseRecycleOriginalPath } from "./file-io.js"

/** v2 ($I) record: header 2, size, time, char-count (incl. null), UTF-16LE path
 *  + a real 2-byte null terminator. */
function v2Record(path: string): Buffer {
  const head = Buffer.alloc(28)
  head.writeUInt32LE(2, 0)
  head.writeUInt32LE(path.length + 1, 24) // char count includes the null
  return Buffer.concat([head, Buffer.from(path, "utf16le"), Buffer.alloc(2)])
}

/** Legacy v1 record: version 1, then a fixed 520-byte (zero-filled) path field —
 *  the zeros after the path act as the null terminator + padding. */
function v1Record(path: string): Buffer {
  const head = Buffer.alloc(24)
  head.writeUInt32LE(1, 0)
  const field = Buffer.alloc(520)
  Buffer.from(path, "utf16le").copy(field)
  return Buffer.concat([head, field])
}

describe("parseRecycleOriginalPath", () => {
  it("reads the v2 (Win10) original path", () => {
    expect(parseRecycleOriginalPath(v2Record("C:\\Users\\me\\a.txt"))).toBe("C:\\Users\\me\\a.txt")
  })

  it("reads the legacy v1 fixed-length path", () => {
    expect(parseRecycleOriginalPath(v1Record("D:\\x\\y.md"))).toBe("D:\\x\\y.md")
  })

  it("returns null for a truncated buffer", () => {
    expect(parseRecycleOriginalPath(Buffer.alloc(10))).toBeNull()
  })

  it("returns null when the declared length overruns the buffer", () => {
    const head = Buffer.alloc(28)
    head.writeUInt32LE(2, 0)
    head.writeUInt32LE(999, 24) // claims far more chars than present
    expect(parseRecycleOriginalPath(head)).toBeNull()
  })
})
