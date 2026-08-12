// Feeds KaminIDE's active Monaco editor (file + selection) to the Bridge webview
// so the composer's "attach file" toggle can append it as `editor-context`.
// This is the KaminIDE equivalent of the original Electron Bridge's FilePanel
// wiring: there the composer sat next to the editor; here Monaco lives in the
// host renderer, so we observe it through the VS Code API and push snapshots.
import * as vscode from "vscode"
import { ipcMain } from "@kaminide/host-compat"
import type { BridgeHost } from "./bridge-host"

// Coalesce the burst of selection events fired on every caret move into one push.
const EDITOR_SELECTION_DEBOUNCE_MS = 120
// Cap the selected text crossing the postMessage boundary — a select-all in a
// large file would otherwise re-serialize the whole buffer to every webview on
// each debounced tick while the selection is held.
const MAX_SELECTION_TEXT = 40_000

interface EditorSelection { path: string; startLine: number; endLine: number; text: string }

/** The active text editor's file + selection, or null when none is active. */
function snapshot(): EditorSelection | null {
  const ed = vscode.window.activeTextEditor
  // Only real on-disk files: untitled / diff / settings buffers have no
  // meaningful path to hand the agent as editor-context.
  if (!ed || ed.document.uri.scheme !== "file") return null
  const s = ed.selection
  const raw = ed.document.getText(s)
  const text = raw.length > MAX_SELECTION_TEXT ? `${raw.slice(0, MAX_SELECTION_TEXT)}\n… [truncated]` : raw
  return {
    path: ed.document.uri.fsPath,
    // VS Code positions are 0-based; the editor-context payload (per CLAUDE.md)
    // is 1-based inclusive line numbers.
    startLine: s.start.line + 1,
    endLine: s.end.line + 1,
    text,
  }
}

/** Observe the active editor + its selection and push snapshots to the webview.
 *  Live changes are broadcast; the INITIAL value is PULLED by the webview on
 *  mount via `get-editor-selection` — webviews attach lazily (after this runs)
 *  and reset their state on the reconnect-reload, so a broadcast seed here would
 *  be dropped. */
export function installEditorContext(host: BridgeHost): vscode.Disposable {
  let timer: ReturnType<typeof setTimeout> | undefined
  const push = (): void => host.broadcast("editor-selection", snapshot())
  const schedule = (): void => {
    if (timer) clearTimeout(timer)
    timer = setTimeout(push, EDITOR_SELECTION_DEBOUNCE_MS)
  }
  ipcMain.handle("get-editor-selection", () => snapshot())
  const subs = [
    vscode.window.onDidChangeActiveTextEditor(schedule),
    vscode.window.onDidChangeTextEditorSelection(schedule),
  ]
  return { dispose: () => { if (timer) clearTimeout(timer); for (const d of subs) d.dispose() } }
}
