// Projects + Sessions store (Phase 2). A project = an explicitly opened folder;
// a session = a named, persisted context inside a project. The renderer's file
// tree follows the active session's project (wired in services/index.ts).
//
// Two notification surfaces, both required:
//   1. broadcast("kamin:sessions:changed", snapshot) → renderer signals.
//   2. an in-process listener bus (onSessionsChange / onActiveSessionChange) —
//      the seam the Phase-3 exthost `kaminide` API subscribes to without a
//      renderer round-trip (mirrors editors.ts's activeListeners pattern).
import { randomUUID } from "node:crypto"
import { readFileSync } from "node:fs"
import { join, normalize } from "node:path"
import type { KaminProject, KaminSession, SessionsSnapshot } from "../../api/types.js"
import { JsonStore } from "../json-store.js"

interface Persisted {
  projects: KaminProject[]
  sessions: KaminSession[]
  activeSessionId: string | null
  /** One-shot marker: the Electron Bridge's saved sessions have been seeded. */
  bridgeImported?: boolean
}

let store: JsonStore<Persisted> | null = null
let state: Persisted = { projects: [], sessions: [], activeSessionId: null }
let broadcast: (channel: string, payload: unknown) => void = () => { /* set in initSessions */ }

const changeListeners = new Set<(snap: SessionsSnapshot) => void>()
const activeListeners = new Set<(session: KaminSession | null) => void>()

export function initSessions(dataDir: string, bc: (channel: string, payload: unknown) => void): void {
  broadcast = bc
  store = new JsonStore<Persisted>(dataDir, "sessions", { projects: [], sessions: [], activeSessionId: null })
  state = { ...store.get() }
  // Invariant the sidebar + file panel depend on: the ACTIVE session is `open`.
  // On a restart the persisted state can restore `activeSessionId` pointing at a
  // session left `open:false` — then the Bridge resumes it (onActive shows the
  // chat) while the tree renders it under "N inactive" and the file panel says
  // "No active session with a folder": the app looks all-inactive with a live
  // chat on screen. Reconcile on load — keep the active session open, or drop a
  // dangling active id — so "active ⟹ open" holds from the first paint.
  if (state.activeSessionId) {
    const act = state.sessions.find((s) => s.id === state.activeSessionId)
    if (!act) state.activeSessionId = null
    else act.open = true // the active session is open by definition
  }
  importBridgeSessions()
}

interface ElectronSaved { conversationId?: string; cwd?: string; label?: string; folderName?: string; lastActivity?: string }

/** One-shot: seed the native project/session tree from the Electron Bridge's
 *  saved sessions so a user upgrading from the Electron app sees their old
 *  projects in the sidebar (the visible tree reads THIS store, not the Bridge
 *  VSIX's own saved-session list). Reads the Electron app's on-disk store
 *  directly — all token buckets, since this tree isn't token-scoped and resume
 *  resolves the JSONL server-side by conversationId. Imported sessions are
 *  inactive (saved, not open tabs), grouped by cwd, and deduped against anything
 *  already carrying the same bridge.conversationId. Runs once (guarded);
 *  best-effort. `force` re-scans even after the one-shot guard — used by the
 *  "remove old Bridge" flow to guarantee nothing is lost before the Electron
 *  config is deleted (idempotent: dedupe by conversationId adds only new ones).
 *  Returns the number of sessions newly imported. */
function importBridgeSessions(force = false): number {
  if (state.bridgeImported && !force) return 0
  const appData = process.env.APPDATA
  const files = appData
    ? [
        join(appData, "open-claude-bridge-electron", "open-claude-bridge-config.json"),
        join(appData, "Electron", "open-claude-bridge-config.json"),
      ]
    : []
  const seen = new Set<string>()
  for (const s of state.sessions) {
    const cid = (s.metadata?.bridge as { conversationId?: string } | undefined)?.conversationId
    if (cid) seen.add(cid)
  }
  const toMs = (v: string | undefined): number => {
    const t = v ? Date.parse(v) : NaN
    return Number.isFinite(t) ? t : Date.now()
  }
  let imported = 0
  for (const file of files) {
    let raw: { savedSessionsByToken?: Record<string, ElectronSaved[]> }
    try { raw = JSON.parse(readFileSync(file, "utf8")) as typeof raw } catch { continue }
    for (const arr of Object.values(raw.savedSessionsByToken ?? {})) {
      if (!Array.isArray(arr)) continue
      for (const es of arr) {
        if (!es.conversationId || seen.has(es.conversationId)) continue
        seen.add(es.conversationId)
        const proj = findOrCreateProject(es.cwd ?? null)
        const when = toMs(es.lastActivity)
        state.sessions = [...state.sessions, {
          id: randomUUID(),
          name: es.label ?? es.folderName ?? "Session",
          projectId: proj.id,
          open: false,
          lastOpened: when,
          createdAt: when,
          metadata: { bridge: { conversationId: es.conversationId } },
        }]
        imported++
      }
    }
  }
  state.bridgeImported = true
  if (imported > 0) {
    emit(false)
    console.info(`[sessions] seeded ${imported} Electron Bridge session(s) into the project tree`)
  } else {
    store?.set(state)
  }
  return imported
}

/** Force a re-scan of the Electron Bridge store and import any sessions not yet
 *  present. Idempotent (deduped by conversationId). Returns the count newly
 *  added. Backs the "remove old Bridge" cleanup: run this before the Electron
 *  config is deleted so no session is lost. */
export function reimportBridgeSessions(): number {
  return importBridgeSessions(true)
}

function snapshot(): SessionsSnapshot {
  return { projects: state.projects, sessions: state.sessions, activeSessionId: state.activeSessionId }
}

/** Persist + fire both notification surfaces. `activeChanged` also fans out the
 *  in-process active-session bus (Phase-3 seam). */
function emit(activeChanged: boolean): void {
  store?.set(state)
  const snap = snapshot()
  broadcast("kamin:sessions:changed", snap)
  for (const fn of changeListeners) fn(snap)
  if (activeChanged) {
    const active = getActiveSession()
    for (const fn of activeListeners) fn(active)
  }
}

export function listSessions(): SessionsSnapshot { return snapshot() }

// Both buses return a Disposable (vscode `Event<T>` convention) so the Phase-3
// `kaminide` namespace can expose them as real `Event<T>` and callers can push
// the result into `context.subscriptions`.
export function onSessionsChange(fn: (snap: SessionsSnapshot) => void): { dispose(): void } {
  changeListeners.add(fn); return { dispose: () => { changeListeners.delete(fn) } }
}
export function onActiveSessionChange(fn: (session: KaminSession | null) => void): { dispose(): void } {
  activeListeners.add(fn); return { dispose: () => { activeListeners.delete(fn) } }
}

export function getActiveSession(): KaminSession | null {
  return state.sessions.find((s) => s.id === state.activeSessionId) ?? null
}

/** Folder the active session's project points at (null = "No folder" or none). */
export function getActiveProjectFolder(): string | null {
  const active = getActiveSession()
  if (!active) return null
  return state.projects.find((p) => p.id === active.projectId)?.folderPath ?? null
}

function findOrCreateProject(folderPath: string | null): KaminProject {
  const norm = folderPath === null ? null : normalize(folderPath)
  const existing = state.projects.find((p) => p.folderPath === norm)
  if (existing) return existing
  const proj: KaminProject = { id: randomUUID(), folderPath: norm, createdAt: Date.now() }
  state.projects = [...state.projects, proj]
  return proj
}

function addSession(projectId: string, name?: string): KaminSession {
  const count = state.sessions.filter((s) => s.projectId === projectId).length
  const session: KaminSession = {
    id: randomUUID(),
    name: name ?? `Session ${String(count + 1)}`,
    projectId,
    open: true, // a freshly created session is active (a titlebar tab)
    lastOpened: Date.now(),
    createdAt: Date.now(),
  }
  state.sessions = [...state.sessions, session]
  state.activeSessionId = session.id
  return session
}

/** New session in `folderPath`'s project (folder deduped, session always added),
 *  activated. Backs the sidebar "New session" (which picks a folder first). */
export function newSessionInFolder(folderPath: string): KaminSession {
  const proj = findOrCreateProject(folderPath)
  const session = addSession(proj.id)
  emit(true)
  return session
}

/** New session under `projectId` (defaults to the active session's project, else
 *  a "No folder" bucket). Becomes active. */
export function newSession(projectId?: string, name?: string): KaminSession {
  let pid = projectId ?? getActiveSession()?.projectId ?? state.projects[0]?.id
  pid ??= findOrCreateProject(null).id
  const session = addSession(pid, name)
  emit(true)
  return session
}

/** New session in the synthetic "No folder" project (created on demand). */
export function newNoFolderSession(): KaminSession {
  const session = addSession(findOrCreateProject(null).id)
  emit(true)
  return session
}

function patchSession(id: string, patch: Partial<KaminSession>): void {
  state.sessions = state.sessions.map((s) => (s.id === id ? { ...s, ...patch } : s))
}

export function renameSession(id: string, name: string): SessionsSnapshot {
  // An explicit user rename is sticky: mark it so the Bridge's CLI auto-title
  // (updateSession below) can't silently overwrite it. Cleared when the user
  // asks to regenerate the title from chat (metadata.nameSetByUser = false).
  const cur = state.sessions.find((s) => s.id === id)
  patchSession(id, { name, metadata: { ...cur?.metadata, nameSetByUser: true } })
  emit(false); return snapshot()
}
/** Persist a session's restorable UI state (open files + panel layout + web URL).
 *  Shallow-merge: only the provided slices are written (the sync sends all three). */
export function setSessionState(
  id: string,
  state: Pick<KaminSession, "editorState" | "layout" | "webUrl">,
): SessionsSnapshot {
  patchSession(id, state); emit(false); return snapshot()
}
export function setSessionColor(id: string, color: string | null): SessionsSnapshot {
  // Clear by omitting the key (exactOptionalPropertyTypes forbids color: undefined).
  state.sessions = state.sessions.map((s) => {
    if (s.id !== id) return s
    const next = { ...s }
    if (color) next.color = color
    else delete next.color
    return next
  })
  emit(false)
  return snapshot()
}
export function setSessionPinned(id: string, pinned: boolean): SessionsSnapshot {
  patchSession(id, { pinned }); emit(false); return snapshot()
}
/** Shallow-merge `name`/`metadata` (the Phase-3 Bridge-VSIX control path). */
export function updateSession(id: string, patch: { name?: string; metadata?: Record<string, unknown> }): KaminSession | null {
  const cur = state.sessions.find((s) => s.id === id)
  if (!cur) return null
  const merged: Partial<KaminSession> = {}
  // Merge metadata FIRST so a same-call flag change (e.g. clearing nameSetByUser
  // alongside a name) is visible to the name guard below. Always an object.
  const nextMeta: Record<string, unknown> = { ...cur.metadata, ...patch.metadata }
  if (patch.metadata !== undefined) merged.metadata = nextMeta
  // Auto-title (the Bridge's only caller with `name`) must not clobber a name
  // the user set by hand — unless that stickiness has been explicitly cleared.
  if (patch.name !== undefined && nextMeta.nameSetByUser !== true) merged.name = patch.name
  patchSession(id, merged)
  emit(false)
  return state.sessions.find((s) => s.id === id) ?? null
}

export function deleteSession(id: string): SessionsSnapshot {
  const wasActive = state.activeSessionId === id
  const victim = state.sessions.find((s) => s.id === id)
  state.sessions = state.sessions.filter((s) => s.id !== id)
  if (wasActive) {
    // Fall back to another session in the same project, else any, else none.
    const sameProject = victim ? state.sessions.find((s) => s.projectId === victim.projectId) : undefined
    state.activeSessionId = (sameProject ?? state.sessions[0])?.id ?? null
  }
  emit(wasActive)
  return snapshot()
}

/** Delete a whole project (folder group) + every session under it. Empty or
 *  not — the sessions' server-side PTYs detach and are reaped server-side. */
export function deleteProject(projectId: string): SessionsSnapshot {
  const active = getActiveSession()
  const removedActive = active?.projectId === projectId
  state.sessions = state.sessions.filter((s) => s.projectId !== projectId)
  state.projects = state.projects.filter((p) => p.id !== projectId)
  if (removedActive) state.activeSessionId = state.sessions[0]?.id ?? null
  emit(removedActive)
  return snapshot()
}

/** Move `id` to sit before `beforeId` in the stored order (null → to the end).
 *  The renderer's titlebar tab strip reorders by dragging; the array order is
 *  the source of truth for tab order. */
export function moveSessionBefore(id: string, beforeId: string | null): SessionsSnapshot {
  const moved = state.sessions.find((s) => s.id === id)
  if (!moved) return snapshot()
  const rest = state.sessions.filter((s) => s.id !== id)
  const idx = beforeId ? rest.findIndex((s) => s.id === beforeId) : -1
  if (idx < 0) rest.push(moved)
  else rest.splice(idx, 0, moved)
  state.sessions = rest
  emit(false)
  return snapshot()
}

export function setActiveSession(id: string | null): SessionsSnapshot {
  if (id !== null && !state.sessions.some((s) => s.id === id)) return snapshot()
  const alreadyOpen = id ? state.sessions.find((s) => s.id === id)?.open === true : true
  if (state.activeSessionId === id && alreadyOpen) return snapshot()
  state.activeSessionId = id
  // Selecting a session also activates it (loads into memory / makes it a tab).
  if (id) patchSession(id, { lastOpened: Date.now(), open: true })
  emit(true)
  return snapshot()
}

/** Deactivate a session: free it from memory (no longer a tab) but keep it
 *  saved. A pinned session stays a "sleeping" tab. If it was the selected one,
 *  selection falls back to another open session (or null). */
export function deactivateSession(id: string): SessionsSnapshot {
  patchSession(id, { open: false })
  const wasActive = state.activeSessionId === id
  if (wasActive) {
    const next = state.sessions.find((s) => s.id !== id && s.open)
    state.activeSessionId = next?.id ?? null
  }
  emit(wasActive)
  return snapshot()
}
