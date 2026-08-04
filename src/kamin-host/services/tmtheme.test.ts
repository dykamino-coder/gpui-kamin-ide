import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, it } from "vitest"
import { readTmThemeSettings } from "./tmtheme.js"

const dir = mkdtempSync(join(tmpdir(), "kamin-tmtheme-"))
function write(name: string, content: string): string {
  const p = join(dir, name)
  writeFileSync(p, content, "utf8")
  return p
}

const PLIST = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>name</key><string>Sample</string>
  <key>settings</key>
  <array>
    <dict>
      <key>settings</key>
      <dict>
        <key>background</key><string>#1e1e1e</string>
        <key>foreground</key><string>#d4d4d4</string>
      </dict>
    </dict>
    <dict>
      <key>scope</key><string>comment</string>
      <key>settings</key>
      <dict>
        <key>foreground</key><string>#6A9955</string>
        <key>fontStyle</key><string>italic</string>
      </dict>
    </dict>
  </array>
</dict>
</plist>`

describe("readTmThemeSettings", () => {
  it("parses an XML plist .tmTheme into the legacy settings array", () => {
    const s = readTmThemeSettings(write("a.tmTheme", PLIST))
    expect(s).toHaveLength(2)
    expect(s[0]?.settings.background).toBe("#1e1e1e")
    expect(s[0]?.settings.foreground).toBe("#d4d4d4")
    expect(s[1]?.scope).toBe("comment")
    expect(s[1]?.settings.foreground).toBe("#6A9955")
    expect(s[1]?.settings.fontStyle).toBe("italic")
  })

  it("decodes XML entities in values", () => {
    const xml = PLIST.replace("comment", "string &amp; punctuation")
    const s = readTmThemeSettings(write("b.tmTheme", xml))
    expect(s[1]?.scope).toBe("string & punctuation")
  })

  it("parses a JSON .tmTheme with a top-level settings array", () => {
    const json = JSON.stringify({ name: "J", settings: [{ settings: { foreground: "#fff" } }, { scope: "keyword", settings: { foreground: "#569cd6" } }] })
    const s = readTmThemeSettings(write("c.json", json))
    expect(s).toHaveLength(2)
    expect(s[1]?.scope).toBe("keyword")
  })

  it("parses a JSON file whose root IS the tokenColors array", () => {
    const json = JSON.stringify([{ scope: "variable", settings: { foreground: "#9cdcfe" } }])
    const s = readTmThemeSettings(write("d.json", json))
    expect(s[0]?.scope).toBe("variable")
  })
})
