// B10 — icon-theme document loading: parse + resolve iconDefinition paths.
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, it } from "vitest"
import { loadIconThemeDoc, readIconSvg } from "./icon-theme.js"

describe("loadIconThemeDoc", () => {
  it("parses the maps and resolves iconDefinition paths to absolute", async () => {
    const dir = await mkdtemp(join(tmpdir(), "kamin-icons-"))
    try {
      const jsonPath = join(dir, "theme.json")
      await writeFile(jsonPath, JSON.stringify({
        iconDefinitions: { _file: { iconPath: "./icons/file.svg" }, _glyph: { fontCharacter: "\\e001" } },
        file: "_file",
        fileExtensions: { ts: "_file" },
        light: { file: "_file" },
      }))
      const doc = await loadIconThemeDoc(jsonPath)
      expect(doc.file).toBe("_file")
      expect(doc.fileExtensions?.ts).toBe("_file")
      expect(doc.iconDefinitions?._file?.iconPath).toBe(join(dir, "icons", "file.svg"))
      expect(doc.iconDefinitions?._glyph?.fontCharacter).toBe("\\e001") // font defs untouched
      expect(doc.light?.file).toBe("_file")
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })

  it("readIconSvg returns a data URL", async () => {
    const dir = await mkdtemp(join(tmpdir(), "kamin-icons-"))
    try {
      const svg = join(dir, "x.svg")
      await writeFile(svg, "<svg xmlns='http://www.w3.org/2000/svg'/>")
      const url = await readIconSvg(svg)
      expect(url.startsWith("data:image/svg+xml;utf8,")).toBe(true)
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })
})
