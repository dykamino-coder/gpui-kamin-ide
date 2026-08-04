// LSP execute-provider commands — the canonical `vscode.execute*Provider`
// surface the claude-bridge VSIX's MCP LSP tools (LspHover/LspDefinition/
// LspReferences) call through `vscode.commands.executeCommand`. Extension and
// host share this process, so the vscode.Uri/Position args arrive as real
// objects (the normalizers below also accept plain shapes in case the exthost
// is ever forked and args cross a serialization boundary).
//
// Why a dedicated language-id resolver instead of the host's `langOf`: a file
// the user hasn't opened isn't in the document mirror, and even
// `openTextDocument(fsPath)` registers it as `plaintext` — so a provider whose
// selector is `{language:'typescript'}` would never match. We resolve the id
// from the contributed-language table (extensions/filenames) so providers fire
// for un-opened files too.

import type { LanguageFeatures } from "./api/language-features.js"
import type { WorkspaceHost } from "./host-services.js"
import type { Registry } from "./registry.js"

const fsPathOf = (u: unknown): string => {
  if (typeof u === "string") return u
  const o = u as { fsPath?: string; path?: string } | null
  return o?.fsPath ?? o?.path ?? String(u)
}
// `p` is whatever an extension passed — the Number() wrapper LOOKED like it was
// coercing, but the cast had already told TS the field was a number, so it only
// hid the check. Read the field as unknown and actually verify it.
const numField = (p: unknown, key: "line" | "character"): number => {
  const v = (p as Record<string, unknown> | null | undefined)?.[key]
  return typeof v === "number" ? v : 0
}
const posLine = (p: unknown): number => numField(p, "line")
const posChar = (p: unknown): number => numField(p, "character")

/** Register the four `vscode.execute*Provider` commands against the shared
 *  registry + languageFeatures. Idempotent per registry (register once). */
export function registerLspCommands(
  registry: Registry,
  languageFeatures: LanguageFeatures,
  workspaceHost: WorkspaceHost,
): void {
  /** Resolve a languageId for a path: prefer the real id if the doc is open
   *  and non-plaintext, else map by contributed filename/extension. */
  const langForUri = (uri: string): string => {
    const open = workspaceHost.documents.get(uri)?.languageId
    if (open && open !== "plaintext") return open
    const lower = uri.toLowerCase()
    const base = lower.split(/[\\/]/).pop() ?? lower
    const ext = base.includes(".") ? `.${  base.split(".").pop()}` : ""
    for (const l of registry.snapshot().languages) {
      if (l.filenames?.some((f) => f.toLowerCase() === base)) return l.id
      if (ext && l.extensions?.some((e) => e.toLowerCase() === ext)) return l.id
    }
    return open ?? "plaintext"
  }

  registry.registerCommand("vscode.executeHoverProvider", (uri, pos) => {
    const fs = fsPathOf(uri)
    return languageFeatures.provideHover(fs, langForUri(fs), posLine(pos), posChar(pos))
  }, { title: "Execute Hover Provider", category: "LSP" })

  registry.registerCommand("vscode.executeDefinitionProvider", (uri, pos) => {
    const fs = fsPathOf(uri)
    return languageFeatures.provideDefinition(fs, langForUri(fs), posLine(pos), posChar(pos))
  }, { title: "Execute Definition Provider", category: "LSP" })

  registry.registerCommand("vscode.executeReferenceProvider", (uri, pos) => {
    const fs = fsPathOf(uri)
    return languageFeatures.provideReferences(fs, langForUri(fs), posLine(pos), posChar(pos), true)
  }, { title: "Execute Reference Provider", category: "LSP" })

  registry.registerCommand("vscode.executeDocumentSymbolProvider", (uri) => {
    const fs = fsPathOf(uri)
    return languageFeatures.provideDocumentSymbols(fs, langForUri(fs))
  }, { title: "Execute Document Symbol Provider", category: "LSP" })
}
