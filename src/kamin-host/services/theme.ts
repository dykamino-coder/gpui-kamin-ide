// Read a contributed color theme file (contributes.themes[].path) for the
// renderer to apply. Themes are JSONC, may `include` a base theme (merged
// base→derived), and carry workbench `colors` + TextMate `tokenColors`.
import { readFileSync } from "node:fs"
import { dirname, resolve } from "node:path"
import { parse as parseJsonc } from "jsonc-parser"
import { readTmThemeSettings } from "./tmtheme.js"

export interface ThemeTokenColor {
  scope?: string | string[]
  settings: { foreground?: string; background?: string; fontStyle?: string }
}

export interface ThemeData {
  /** Light vs dark base — drives the Monaco base + the renderer's data-theme. */
  type: "dark" | "light"
  /** Workbench colour map (e.g. "editor.background" → "#282a36"). */
  colors: Record<string, string>
  /** TextMate token rules for the editor. */
  tokenColors: ThemeTokenColor[]
}

interface RawTheme {
  type: unknown
  colors: Record<string, string>
  tokenColors: ThemeTokenColor[]
}

// VS Code: a colour whose value is exactly "default" means "remove this key, fall
// back to the registry default" (used by include-based themes to reset a base key).
const DEFAULT_COLOR_VALUE = "default"

// NOTE: `semanticTokenColors` / `semanticHighlighting` are intentionally NOT
// forwarded. Monaco's standalone theme API (`defineTheme`) has no semantic-colour
// hook — semantic highlighting needs a registered DocumentSemanticTokensProvider
// per language AND a Monaco that consumes the colours (it doesn't, standalone).
// TextMate `tokenColors` already cover ~95% of highlighting; full semantic theming
// is a separate feature (provider pipeline), not a theme-parser gap.
export function readThemeFile(path: string): ThemeData {
  const raw = readOne(path, new Set())
  const isLight = raw.type === "light" || raw.type === "vs"
  return { type: isLight ? "light" : "dark", colors: raw.colors, tokenColors: raw.tokenColors }
}

/** Merge `override` colours over `base`, honouring the "default" delete sentinel. */
function mergeColors(base: Record<string, string>, override: unknown): Record<string, string> {
  const merged: Record<string, string> = { ...base }
  const removed = new Set<string>()
  if (override && typeof override === "object") {
    for (const [k, v] of Object.entries(override as Record<string, unknown>)) {
      if (v === DEFAULT_COLOR_VALUE) removed.add(k)
      else if (typeof v === "string") merged[k] = v
    }
  }
  return Object.fromEntries(Object.entries(merged).filter(([k]) => !removed.has(k)))
}

/** Legacy flat-`settings`-array theme format → workbench colours + token rules.
 *  Minimal mapping (the no-scope global entry → editor fg/bg; scoped entries →
 *  tokenColors). Not the full VS Code `settingToColorIdMapping`, but enough that a
 *  legacy theme renders instead of coming out blank. */
function convertLegacySettings(settings: ThemeTokenColor[]): { colors: Record<string, string>; tokenColors: ThemeTokenColor[] } {
  const colors: Record<string, string> = {}
  const tokenColors: ThemeTokenColor[] = []
  for (const s of settings) {
    if (!s.scope) {
      if (s.settings.background) colors["editor.background"] = s.settings.background
      if (s.settings.foreground) colors["editor.foreground"] = s.settings.foreground
    } else {
      tokenColors.push({ scope: s.scope, settings: s.settings })
    }
  }
  return { colors, tokenColors }
}

/** Read one theme file, recursively merging its `include` base first. */
function readOne(path: string, seen: Set<string>): RawTheme {
  const abs = resolve(path)
  if (seen.has(abs)) return { type: undefined, colors: {}, tokenColors: [] }
  seen.add(abs)
  const json = parseJsonc(readFileSync(abs, "utf8")) as Record<string, unknown>
  const base = typeof json.include === "string"
    ? readOne(resolve(dirname(abs), json.include), seen)
    : { type: undefined as unknown, colors: {}, tokenColors: [] as ThemeTokenColor[] }

  // Legacy format: a top-level `settings` array carries both colours + tokens.
  if (Array.isArray(json.settings)) {
    const conv = convertLegacySettings(json.settings as ThemeTokenColor[])
    return { type: json.type ?? base.type, colors: mergeColors(base.colors, conv.colors), tokenColors: [...base.tokenColors, ...conv.tokenColors] }
  }

  // `tokenColors` may be an array OR a string path to a `.tmTheme` (JSON or XML
  // plist). For the string form, the .tmTheme `settings` array carries BOTH the
  // global editor colours and the scoped token rules (legacy shape) — convert it.
  let colors = mergeColors(base.colors, json.colors)
  let tokenColors: ThemeTokenColor[] = []
  if (Array.isArray(json.tokenColors)) tokenColors = json.tokenColors as ThemeTokenColor[]
  else if (typeof json.tokenColors === "string") {
    try {
      const conv = convertLegacySettings(readTmThemeSettings(resolve(dirname(abs), json.tokenColors)))
      colors = mergeColors(colors, conv.colors)
      tokenColors = conv.tokenColors
    } catch (err) {
      console.warn(`[kamin-host] theme ${abs}: failed to read .tmTheme "${json.tokenColors}" — syntax colours missing.`, err)
    }
  }

  return {
    type: json.type ?? base.type,
    colors,
    tokenColors: [...base.tokenColors, ...tokenColors],
  }
}
