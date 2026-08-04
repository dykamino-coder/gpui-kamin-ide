// Renderer-marshalled Uri revival. The renderer passes a resource to an
// explorer/context command as `{ $mid: 1, fsPath }` (vscode's Uri marshal
// shape); turn it back into a real vscode `Uri` so extension handlers can call
// `.fsPath`/`.toString()`/`.with()`. Other args pass through untouched.
//
// Lives next to the ext-host child because that is where executeCommand now
// runs (after the crash-isolation split) — the revive must happen AFTER the
// fork-IPC hop, since a real Uri instance would not survive structured clone.
import { URI } from "vscode-uri"

export function reviveCommandArg(arg: unknown): unknown {
  if (arg && typeof arg === "object" && (arg as { $mid?: unknown }).$mid === 1) {
    const fsPath = (arg as { fsPath?: unknown }).fsPath
    if (typeof fsPath === "string") return URI.file(fsPath)
  }
  return arg
}
