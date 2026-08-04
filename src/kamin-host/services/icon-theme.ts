// File icon theme loading (B10) — reads a VS Code `contributes.iconThemes`
// JSON document and serves its SVG icons. The resolution (name → definition id)
// happens renderer-side; the host just parses the document, resolves icon paths
// to absolute, and reads SVG bytes on demand. Matches VS Code's iconTheme JSON
// shape (iconDefinitions + file/folder/*Names/*Extensions/languageIds maps +
// light/highContrast override maps).
import { readFile } from "node:fs/promises"
import { dirname, resolve } from "node:path"

interface IconDefinition { iconPath?: string; fontCharacter?: string; fontColor?: string; fontId?: string }

/** Map fields shared by the top-level doc and its light/highContrast overrides
 *  (the overrides reference the SAME top-level iconDefinitions). */
interface IconThemeMaps {
  file?: string
  folder?: string
  folderExpanded?: string
  rootFolder?: string
  rootFolderExpanded?: string
  fileExtensions?: Record<string, string>
  fileNames?: Record<string, string>
  folderNames?: Record<string, string>
  folderNamesExpanded?: Record<string, string>
  rootFolderNames?: Record<string, string>
  rootFolderNamesExpanded?: Record<string, string>
  languageIds?: Record<string, string>
}

export interface IconThemeDoc extends IconThemeMaps {
  iconDefinitions?: Record<string, IconDefinition>
  light?: IconThemeMaps
  highContrast?: IconThemeMaps
  hidesExplorerArrows?: boolean
}

/** Parse a theme JSON; iconDefinition `iconPath`s become absolute (resolved
 *  against the JSON's own directory, as VS Code does). */
export async function loadIconThemeDoc(jsonPath: string): Promise<IconThemeDoc> {
  const doc = JSON.parse(await readFile(jsonPath, "utf8")) as IconThemeDoc
  const base = dirname(jsonPath)
  const defs = doc.iconDefinitions
  if (defs && typeof defs === "object") {
    for (const id of Object.keys(defs)) {
      const d = defs[id]
      if (d && typeof d.iconPath === "string") d.iconPath = resolve(base, d.iconPath)
    }
  }
  return doc
}

// data-URL cache keyed by absolute SVG path (icons are few and reused).
const iconUrlCache = new Map<string, string>()

/** Read one icon SVG → a `data:` URL the renderer can use as an <img> src. */
export async function readIconSvg(absPath: string): Promise<string> {
  const cached = iconUrlCache.get(absPath)
  if (cached !== undefined) return cached
  const svg = await readFile(absPath, "utf8")
  const url = `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`
  iconUrlCache.set(absPath, url)
  return url
}
