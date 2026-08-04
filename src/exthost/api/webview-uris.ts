// Resource plumbing for webviews (extracted from webview.ts): the custom-protocol
// origin they're served from, and how a local file becomes a URL the iframe may
// load. Pure — no registry state.
import type { URI } from "vscode-uri"
import { Uri } from "./shared.js"

// B7c-2: webviews are served from this custom-protocol origin (see
// src-tauri/src/webview.rs). asWebviewUri maps a local file to a same-origin
// `/__resource/<id>/…` URL, and cspSource is this origin so the extension's own
// CSP meta tag permits those resources.
const WEBVIEW_ORIGIN_AUTHORITY = "kaminwebview.localhost"
export const WEBVIEW_CSP_SOURCE = `http://${WEBVIEW_ORIGIN_AUTHORITY}`

/** `webview.asWebviewUri` for one webview id → a same-origin resource URL the
 *  Rust handler serves from disk (confined to the webview's localResourceRoots).
 *  `uri.path` is already `/C:/…` (Windows) or `/abs/…` (posix), so we just
 *  prefix the route; URI.toString() percent-encodes it for the iframe. */
export function asWebviewUriFor(id: string): (uri: URI) => URI {
  return (uri) => Uri.from({ scheme: "http", authority: WEBVIEW_ORIGIN_AUTHORITY, path: `/__resource/${id}${uri.path}` })
}

/** Resolve the effective localResourceRoots: the extension-provided list, or
 *  the VS Code default (extension install dir + workspace folders). Returns both
 *  the Uri[] the extension sees and the fs paths the renderer forwards to Rust. */
export function resolveRoots(provided: readonly URI[] | undefined, extensionRoot: string | undefined, workspaceRoots: readonly string[]): { uris: URI[]; fsPaths: string[] } {
  const uris = provided !== undefined
    ? [...provided]
    : [...(extensionRoot ? [Uri.file(extensionRoot)] : []), ...workspaceRoots.map((w) => Uri.file(w))]
  return { uris, fsPaths: uris.map((u) => u.fsPath) }
}
