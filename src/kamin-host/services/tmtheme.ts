// Parse a TextMate `.tmTheme` file referenced by a theme's STRING `tokenColors`
// (VS Code supports both the inline array and a path to a `.tmTheme`). The file
// is either JSON or an Apple XML plist; both carry a `settings` array in the
// legacy theme shape ({ scope?, settings: { foreground/background/fontStyle } }).
import { readFileSync } from "node:fs"
import { parse as parseJsonc } from "jsonc-parser"
import type { ThemeTokenColor } from "./theme.js"

type PlistValue = string | number | boolean | PlistValue[] | { [k: string]: PlistValue }
interface Tok { type: "open" | "close" | "self" | "text"; name?: string; text?: string }

function decodeEntities(s: string): string {
  return s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&amp;/g, "&")
}

/** Tokenise plist XML into open/close/self/text events; XML decl, DOCTYPE and
 *  comments match but capture nothing → skipped. */
function tokenize(xml: string): Tok[] {
  const out: Tok[] = []
  const re = /<!--[\s\S]*?-->|<!\[CDATA\[([\s\S]*?)\]\]>|<\?[\s\S]*?\?>|<![^>]*>|<\/([a-zA-Z]+)\s*>|<([a-zA-Z]+)[^>]*?(\/?)>|([^<]+)/g
  let m: RegExpExecArray | null
  while ((m = re.exec(xml))) {
    if (m[1] !== undefined) out.push({ type: "text", text: m[1] })       // CDATA
    else if (m[2]) out.push({ type: "close", name: m[2] })
    else if (m[3]) out.push({ type: m[4] === "/" ? "self" : "open", name: m[3] })
    else if (m[5] !== undefined) out.push({ type: "text", text: m[5] })
  }
  return out
}

function parsePlist(xml: string): PlistValue | undefined {
  const toks = tokenize(xml)
  let i = 0
  const skipText = (): void => { while (toks[i]?.type === "text") i++ }
  const text = (): string => { let s = ""; while (toks[i]?.type === "text") { s += toks[i]?.text ?? ""; i++ } return decodeEntities(s) }
  const close = (name: string): void => { if (toks[i]?.type === "close" && toks[i]?.name === name) i++ }

  function value(): PlistValue | undefined {
    skipText()
    const t = toks[i]
    if (!t) return undefined
    if (t.type === "self") { i++; return t.name === "true" ? true : t.name === "false" ? false : "" }
    if (t.type !== "open" || !t.name) return undefined
    const name = t.name; i++
    if (name === "dict") {
      const obj: Record<string, PlistValue> = {}
      for (;;) {
        skipText()
        if (!toks[i] || (toks[i]?.type === "close" && toks[i]?.name === "dict")) { i++; break }
        if (toks[i]?.type === "open" && toks[i]?.name === "key") { i++; const k = text(); close("key"); const v = value(); if (v !== undefined) obj[k] = v }
        else i++ // unexpected — skip
      }
      return obj
    }
    if (name === "array") {
      const arr: PlistValue[] = []
      for (;;) {
        skipText()
        if (!toks[i] || (toks[i]?.type === "close" && toks[i]?.name === "array")) { i++; break }
        const v = value(); if (v !== undefined) arr.push(v)
      }
      return arr
    }
    const raw = text(); close(name)
    return name === "integer" || name === "real" ? Number(raw) : raw
  }

  while (toks[i] && !(toks[i]?.type === "open" && toks[i]?.name === "plist")) i++
  if (toks[i]) i++ // consume <plist>
  return value()
}

/** Read a `.tmTheme` (JSON or XML plist) → its `settings` array (legacy shape). */
export function readTmThemeSettings(path: string): ThemeTokenColor[] {
  const content = readFileSync(path, "utf8")
  const head = content.trimStart()
  if (head.startsWith("{") || head.startsWith("[")) {
    const j = parseJsonc(content) as unknown
    const arr = Array.isArray(j) ? j
      : j && typeof j === "object" && Array.isArray((j as Record<string, unknown>).settings) ? (j as Record<string, unknown>).settings
      : j && typeof j === "object" && Array.isArray((j as Record<string, unknown>).tokenColors) ? (j as Record<string, unknown>).tokenColors
      : []
    return arr as ThemeTokenColor[]
  }
  const root = parsePlist(content)
  const settings = root && typeof root === "object" && !Array.isArray(root) ? root.settings : undefined
  return Array.isArray(settings) ? settings as unknown as ThemeTokenColor[] : []
}
