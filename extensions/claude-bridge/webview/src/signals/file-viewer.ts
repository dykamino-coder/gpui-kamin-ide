import { storage } from "@bridge/storage"
// Per-chat-tab file viewer state — which files are open, which one is
// active, and the in-memory CodeMirror EditorState/transaction history
// per (tabId, filePath) so undo/redo + scroll position survive tab and
// file switches without resetting.
//
// Only the path lists persist via electron-store between sessions.
// Editor state itself (history stack, scroll, selection) is renderer-
// only — restoring file content from disk on next session is enough,
// undo across reloads would require a much bigger persistence model.

import { signal, computed, effect } from '@preact/signals'
import type { EditorState } from '@codemirror/state'
import { activeTabId, tabs } from './tabs'

export interface OpenFile {
  /** Absolute path on disk. */
  path: string
  /** mtimeMs of the on-disk version we last read/wrote — compared
   *  against incoming `external-change` events to ignore self-saves. */
  mtimeMs: number
  /** True while an autosave is in flight. Drives the dot-on-tab
   *  "unsaved" indicator. */
  dirty: boolean
}

/** Per-(chat tab) list of open file paths + which one is active. */
export const openFilesByTab = signal<Map<string, OpenFile[]>>(new Map())
export const activeFileByTab = signal<Map<string, string | null>>(new Map())

export interface SelectionInfo {
  /** Path the selection lives in. */
  path: string
  /** 1-based line number range (inclusive). startLine === endLine for
   *  single-line selections. */
  startLine: number
  endLine: number
  /** Selected text (may be empty when caret only). */
  text: string
}
/** Latest selection per chat tab — refreshed on every CodeMirror cursor
 *  change. Used by the input-bar attach toggle so the message can carry
 *  the active file + highlighted lines as context to the agent. */
export const activeSelectionByTab = signal<Map<string, SelectionInfo | null>>(new Map())

export function setActiveSelection(tabId: string, info: SelectionInfo | null): void {
  const next = new Map(activeSelectionByTab.value)
  next.set(tabId, info)
  activeSelectionByTab.value = next
}

/** The active file + selection in KaminIDE's HOST Monaco editor, pushed by the
 *  Bridge extension (which observes `vscode.window.activeTextEditor` +
 *  `onDidChangeTextEditorSelection`). In KaminIDE the host editor is THE editor,
 *  so it wins over the legacy per-tab CodeMirror selection below; null when no
 *  text editor is active. */
export const hostEditorSelection = signal<SelectionInfo | null>(null)

export const activeSelection = computed<SelectionInfo | null>(() => {
  if (hostEditorSelection.value) return hostEditorSelection.value
  // Fallback: the legacy per-tab CodeMirror selection (setActiveSelection). Only
  // FileEditor/FilePanel write it, and neither is mounted in KaminIDE (Monaco is
  // the editor) — this branch is dead here, kept for the Electron build only.
  const id = activeTabId.value
  return (id ? activeSelectionByTab.value.get(id) : null) ?? null
})

/** User toggle (per-app, persisted) — when ON, every chat message
 *  automatically appends the active file's path + selected lines (or
 *  caret position) to the message body. Off by default. */
const _initialAttach = storage.getItem('attach-active-file') === 'true'
export const attachActiveFile = signal<boolean>(_initialAttach)
export function setAttachActiveFile(on: boolean): void {
  attachActiveFile.value = on
  try { storage.setItem('attach-active-file', String(on)) } catch { /* noop */ }
}

/** Per-file view mode: code (CodeMirror) or preview (rendered).
 *  Applies to file types that have a sensible rendered form — HTML,
 *  Markdown, SVG, etc. Default is `code`. Keyed by absolute file path
 *  so the same file remembers its mode across chats and across app
 *  restarts (persisted in localStorage). The `(tabId, filePath)`
 *  function signature is kept for API symmetry with other helpers in
 *  this file even though tabId is unused. */
export type FileViewMode = 'code' | 'preview'

const PREVIEW_MODE_STORAGE_KEY = 'preview-mode-by-path-v1'
function loadPreviewModes(): Record<string, FileViewMode> {
  try {
    const raw = storage.getItem(PREVIEW_MODE_STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw)
    return typeof parsed === 'object' && parsed ? parsed : {}
  } catch { return {} }
}
function savePreviewModes(map: Map<string, FileViewMode>): void {
  try {
    const obj: Record<string, FileViewMode> = {}
    for (const [k, v] of map) obj[k] = v
    storage.setItem(PREVIEW_MODE_STORAGE_KEY, JSON.stringify(obj))
  } catch { /* quota / privacy mode */ }
}

export const previewModeByPath = signal<Map<string, FileViewMode>>(
  new Map(Object.entries(loadPreviewModes())),
)

export function getPreviewMode(_tabId: string, filePath: string): FileViewMode {
  return previewModeByPath.value.get(filePath) ?? 'code'
}
export function setPreviewMode(_tabId: string, filePath: string, mode: FileViewMode): void {
  const next = new Map(previewModeByPath.value)
  next.set(filePath, mode)
  previewModeByPath.value = next
  savePreviewModes(next)
}

/** @deprecated alias kept so any older import keeps compiling. */
export const previewModeByKey = previewModeByPath

/** Module-level Map from `${tabId}::${path}` → live CodeMirror EditorState.
 *  Kept outside the signal because EditorState is too heavy to compare
 *  on every render and we never want preact to subscribe to it. The
 *  editor component reads this directly when swapping files. */
export const editorStates = new Map<string, EditorState>()

export function makeStateKey(tabId: string, filePath: string): string {
  return `${tabId}::${filePath}`
}

export function getOpenFiles(tabId: string): OpenFile[] {
  return openFilesByTab.value.get(tabId) ?? []
}
export function getActiveFile(tabId: string): string | null {
  return activeFileByTab.value.get(tabId) ?? null
}

export function setOpenFiles(tabId: string, files: OpenFile[]): void {
  const next = new Map(openFilesByTab.value)
  next.set(tabId, files)
  openFilesByTab.value = next
}
export function setActiveFile(tabId: string, filePath: string | null): void {
  const next = new Map(activeFileByTab.value)
  next.set(tabId, filePath)
  activeFileByTab.value = next
}

/** Add `filePath` to the tab's open-files list (if absent) and make it
 *  active. mtimeMs/content are fetched lazily by the editor. */
export function openFileInTab(tabId: string, filePath: string): void {
  const existing = getOpenFiles(tabId)
  if (!existing.find(f => f.path === filePath)) {
    setOpenFiles(tabId, [...existing, { path: filePath, mtimeMs: 0, dirty: false }])
  }
  setActiveFile(tabId, filePath)
}

export function closeFileInTab(tabId: string, filePath: string): void {
  const existing = getOpenFiles(tabId)
  const idx = existing.findIndex(f => f.path === filePath)
  if (idx < 0) return
  const next = existing.slice()
  next.splice(idx, 1)
  setOpenFiles(tabId, next)
  // Drop the editor state cache for this file so a subsequent reopen
  // starts from a fresh document instead of resurrecting stale text.
  editorStates.delete(makeStateKey(tabId, filePath))
  // If we closed the active file, fall back to the neighbour (right,
  // then left) — matches VSCode/JetBrains tab-close behaviour.
  if (getActiveFile(tabId) === filePath) {
    const fallback = next[idx]?.path ?? next[idx - 1]?.path ?? null
    setActiveFile(tabId, fallback)
  }
}

export function markFileMtime(tabId: string, filePath: string, mtimeMs: number): void {
  const existing = getOpenFiles(tabId)
  const idx = existing.findIndex(f => f.path === filePath)
  if (idx < 0) return
  const next = existing.slice()
  next[idx] = { ...next[idx]!, mtimeMs }
  setOpenFiles(tabId, next)
}

export function markFileDirty(tabId: string, filePath: string, dirty: boolean): void {
  const existing = getOpenFiles(tabId)
  const idx = existing.findIndex(f => f.path === filePath)
  if (idx < 0) return
  if (existing[idx]!.dirty === dirty) return
  const next = existing.slice()
  next[idx] = { ...next[idx]!, dirty }
  setOpenFiles(tabId, next)
}

export const activeOpenFiles = computed<OpenFile[]>(() => {
  const id = activeTabId.value
  if (!id) return []
  return openFilesByTab.value.get(id) ?? []
})
export const activeOpenFilePath = computed<string | null>(() => {
  const id = activeTabId.value
  if (!id) return null
  return activeFileByTab.value.get(id) ?? null
})

/** Close every open file except `keepPath` in the given tab. Used by
 *  the file-tab context menu's "Close others" command. */
export function closeOthersInTab(tabId: string, keepPath: string): void {
  const existing = getOpenFiles(tabId)
  const next = existing.filter(f => f.path === keepPath)
  // Drop cached EditorState for every removed file so a subsequent
  // reopen reads fresh from disk + starts a clean undo history.
  for (const f of existing) {
    if (f.path !== keepPath) editorStates.delete(makeStateKey(tabId, f.path))
  }
  setOpenFiles(tabId, next)
  if (getActiveFile(tabId) !== keepPath) setActiveFile(tabId, keepPath)
}

/** Close every open file positioned to the right of `pivotPath` in the
 *  current visual order. */
export function closeToRightInTab(tabId: string, pivotPath: string): void {
  const existing = getOpenFiles(tabId)
  const idx = existing.findIndex(f => f.path === pivotPath)
  if (idx < 0) return
  const keep = existing.slice(0, idx + 1)
  for (const f of existing.slice(idx + 1)) {
    editorStates.delete(makeStateKey(tabId, f.path))
  }
  setOpenFiles(tabId, keep)
  // If the active file was among the closed ones, fall back to pivot.
  const active = getActiveFile(tabId)
  if (active && !keep.some(f => f.path === active)) setActiveFile(tabId, pivotPath)
}

/** Close every open file in the given tab. */
export function closeAllInTab(tabId: string): void {
  const existing = getOpenFiles(tabId)
  for (const f of existing) editorStates.delete(makeStateKey(tabId, f.path))
  setOpenFiles(tabId, [])
  setActiveFile(tabId, null)
}

// ─── Persistence (per conversationId) ──────────────────────────────
//
// Open-files lists are keyed by the chat tab's stable conversationId
// (NOT the live tabId, which can change between sessions). On startup
// we hydrate the live Map<tabId, …> as new tabs appear; on every change
// we snapshot all open tabs back to storage. Editor state itself
// (undo/redo, scroll, selection) is intentionally *not* persisted —
// reloading file content from disk is a clean enough restore.

const PERSIST_KEY = 'file-viewer-state-v1'
type PersistedFileViewerState = Record<string, { openFiles: string[]; activeFile: string | null }>

function loadPersisted(): PersistedFileViewerState {
  try {
    const raw = storage.getItem(PERSIST_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw)
    return typeof parsed === 'object' && parsed ? parsed : {}
  } catch { return {} }
}

function savePersisted(state: PersistedFileViewerState): void {
  try { storage.setItem(PERSIST_KEY, JSON.stringify(state)) } catch { /* quota / privacy mode */ }
}

let persistTimer: ReturnType<typeof setTimeout> | null = null
function scheduleSnapshot(): void {
  if (persistTimer) clearTimeout(persistTimer)
  persistTimer = setTimeout(() => {
    persistTimer = null
    const state = loadPersisted()
    for (const [tabId, files] of openFilesByTab.value) {
      const tab = tabs.value.find(t => t.id === tabId)
      const convId = tab?.conversationId
      if (!convId) continue
      state[convId] = {
        openFiles: files.map(f => f.path),
        activeFile: activeFileByTab.value.get(tabId) ?? null,
      }
    }
    savePersisted(state)
  }, 350)
}

// Snapshot whenever the open-files set or active-file pointer changes.
// `effect` from @preact/signals re-runs on every dependent signal flip;
// schedule a debounced write to keep storage IO bounded under heavy
// editor activity.
effect(() => {
  // Touch both signals so the effect tracks them.
  void openFilesByTab.value
  void activeFileByTab.value
  scheduleSnapshot()
})

// Hydrate any tab that lands in `tabs.value` with a recoverable
// conversationId — but only once per live tabId so a user-driven close
// doesn't get clobbered by a re-hydration.
const hydratedTabs = new Set<string>()
effect(() => {
  const persisted = loadPersisted()
  for (const tab of tabs.value) {
    if (hydratedTabs.has(tab.id)) continue
    const convId = tab.conversationId
    if (!convId) continue
    const saved = persisted[convId]
    hydratedTabs.add(tab.id)
    if (!saved || saved.openFiles.length === 0) continue
    setOpenFiles(tab.id, saved.openFiles.map(p => ({ path: p, mtimeMs: 0, dirty: false })))
    if (saved.activeFile && saved.openFiles.includes(saved.activeFile)) {
      setActiveFile(tab.id, saved.activeFile)
    } else {
      setActiveFile(tab.id, saved.openFiles[0] ?? null)
    }
  }
})
