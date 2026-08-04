// kaminide.sessions registry (Phase 3). A single host-wide instance backs the
// `kaminide` virtual module's session/project surface: snapshot getters, typed
// change events (re-fired from the in-process SessionsHost bus), and control
// methods. Mirrors the Diagnostics registry + the editors.ts listener-bridge so
// the future Bridge VSIX can drive sessions with real vscode `Event<T>`.
import type { KaminProject, KaminSession, SessionsSnapshot } from "../../api/types.js"
import type { SessionsHost } from "../host-services.js"
import { EventEmitter } from "./shared.js"

/** vscode-style change event — added/removed/changed sessions since the last fire. */
export interface SessionsChangeEvent {
  readonly added: readonly KaminSession[]
  readonly removed: readonly KaminSession[]
  readonly changed: readonly KaminSession[]
}

/** Stable serialization of free-form metadata — sorts the top-level keys so two
 *  equal objects built in different insertion order (e.g. a spread merge) don't
 *  read as different and fan out a spurious `changed` event. */
function metaStr(m: Record<string, unknown> | undefined): string {
  if (!m) return ""
  return JSON.stringify(Object.keys(m).sort().map((k) => [k, m[k]]))
}

/** Fields whose change is meaningful to an extension. `editorState`/`createdAt`
 *  are excluded — open-file churn shouldn't fan out a session-changed event. */
function sessionEq(a: KaminSession, b: KaminSession): boolean {
  return a.name === b.name && a.projectId === b.projectId && a.color === b.color
    && a.pinned === b.pinned && a.open === b.open && a.lastOpened === b.lastOpened
    && metaStr(a.metadata) === metaStr(b.metadata)
}

/** Projects are immutable once created (a folder change makes a NEW project with
 *  a new id, never an in-place edit), so id-membership equality is sufficient. */
function sameProjectIds(a: readonly KaminProject[], b: readonly KaminProject[]): boolean {
  if (a.length !== b.length) return false
  const ids = new Set(a.map((p) => p.id))
  return b.every((p) => ids.has(p.id))
}

export class SessionsApi {
  private readonly sessionsEmitter = new EventEmitter<SessionsChangeEvent>()
  private readonly activeEmitter = new EventEmitter<KaminSession | undefined>()
  private readonly projectsEmitter = new EventEmitter<readonly KaminProject[]>()
  /** Real `vscode.Event<T>` — the returned Disposable goes into `subscriptions`. */
  readonly onDidChangeSessions = this.sessionsEmitter.event
  readonly onDidChangeActiveSession = this.activeEmitter.event
  readonly onDidChangeProjects = this.projectsEmitter.event
  private snap: SessionsSnapshot
  private readonly subs: { dispose(): void }[]

  constructor(private readonly host: SessionsHost) {
    this.snap = host.list()
    // Bridge the in-process host bus once → diff into typed vscode events. Held
    // so dispose() can detach them (the host bus outlives a host teardown).
    this.subs = [
      host.onChange((next) => { this.diff(next) }),
      host.onActiveChange((session) => { this.activeEmitter.fire(session ?? undefined) }),
    ]
  }

  dispose(): void { for (const s of this.subs) s.dispose() }

  private diff(next: SessionsSnapshot): void {
    const prev = new Map(this.snap.sessions.map((s) => [s.id, s]))
    const nextIds = new Set(next.sessions.map((s) => s.id))
    const added: KaminSession[] = []
    const changed: KaminSession[] = []
    for (const s of next.sessions) {
      const before = prev.get(s.id)
      if (!before) added.push(s)
      else if (!sessionEq(before, s)) changed.push(s)
    }
    const removed = this.snap.sessions.filter((s) => !nextIds.has(s.id))
    const projectsChanged = !sameProjectIds(this.snap.projects, next.projects)
    this.snap = next
    if (added.length > 0 || removed.length > 0 || changed.length > 0) {
      this.sessionsEmitter.fire({ added, removed, changed })
    }
    if (projectsChanged) this.projectsEmitter.fire(next.projects)
  }

  // ── Snapshot getters — served from the captured `snap`, so a read inside an
  //    onDidChange* handler sees exactly the snapshot that fired the event. ──
  get all(): readonly KaminSession[] { return this.snap.sessions }
  get projects(): readonly KaminProject[] { return this.snap.projects }
  get active(): KaminSession | undefined {
    return this.snap.sessions.find((s) => s.id === this.snap.activeSessionId) ?? undefined
  }
  getSession(id: string): KaminSession | undefined {
    return this.snap.sessions.find((s) => s.id === id)
  }

  // ── Control (round-trips the host store, which broadcasts the change). The
  //    value-returning mutators are async — the store may live in the parent
  //    process when the ext-host is forked for crash isolation. ──
  createSession(opts?: { projectId?: string; name?: string }): Promise<KaminSession> {
    return this.host.createSession(opts ?? {})
  }
  setActiveSession(id: string): void { this.host.setActiveSession(id) }
  async updateSession(id: string, patch: { name?: string; metadata?: Record<string, unknown> }): Promise<KaminSession | undefined> {
    return (await this.host.updateSession(id, patch)) ?? undefined
  }
  deleteSession(id: string): void { this.host.deleteSession(id) }
}
