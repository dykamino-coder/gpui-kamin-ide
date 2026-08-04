// Ambient type contract for the `kaminide` virtual module (Phase 3) — the
// extension-author-facing `.d.ts`, the way `@types/vscode` types `require('vscode')`.
// It is NOT imported by the host (the runtime shape lives in api-kaminide.ts +
// api/sessions.ts); it only gives TS extension authors `require('kaminide')`
// types. Keep it in sync with `KaminProject`/`KaminSession` in src/api/types.ts
// and the `SessionsApi` surface.
declare module "kaminide" {
  /** A project = an explicitly opened folder (null folderPath = the "No folder" bucket). */
  export interface KaminProject {
    readonly id: string
    readonly folderPath: string | null
    readonly createdAt: number
  }

  /** A session = a named, persisted context belonging to a project. */
  export interface KaminSession {
    readonly id: string
    readonly name: string
    readonly projectId: string
    /** User-assignable accent colour (palette swatch). */
    readonly color?: string
    /** Shown in the top pinned-sessions bar. */
    readonly pinned?: boolean
    /** Loaded into memory (a titlebar tab). */
    readonly open?: boolean
    readonly lastOpened: number
    readonly createdAt: number
    /** Free-form data an extension (e.g. the Bridge VSIX) attaches to a session. */
    readonly metadata?: Record<string, unknown>
  }

  export interface SessionsChangeEvent {
    readonly added: readonly KaminSession[]
    readonly removed: readonly KaminSession[]
    readonly changed: readonly KaminSession[]
  }

  export interface Disposable { dispose(): void }

  /** Mirrors `vscode.Event<T>` — the returned Disposable goes into subscriptions. */
  export type Event<T> = (listener: (e: T) => unknown, thisArgs?: unknown, disposables?: Disposable[]) => Disposable

  export interface SessionsApi {
    /** All sessions, live. */
    readonly all: readonly KaminSession[]
    /** All projects, live. */
    readonly projects: readonly KaminProject[]
    /** The active session, or undefined. */
    readonly active: KaminSession | undefined
    getSession(id: string): KaminSession | undefined
    /** Delta of session changes since the last fire. */
    readonly onDidChangeSessions: Event<SessionsChangeEvent>
    readonly onDidChangeActiveSession: Event<KaminSession | undefined>
    /** The FULL project list after any change (snapshot, not a delta — projects
     *  are a short list, so listeners get the whole array). */
    readonly onDidChangeProjects: Event<readonly KaminProject[]>
    /** Create + activate a session (in `projectId`, else the active/No-folder
     *  project). Async — the sessions store round-trips the host (and, with
     *  crash isolation, the parent process). */
    createSession(opts?: { projectId?: string; name?: string }): Promise<KaminSession>
    setActiveSession(id: string): void
    updateSession(id: string, patch: { name?: string; metadata?: Record<string, unknown> }): Promise<KaminSession | undefined>
    deleteSession(id: string): void
  }

  export const version: string
  export const sessions: SessionsApi
}
