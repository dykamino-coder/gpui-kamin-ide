// Local copy of KaminIDE's `kaminide` virtual-module contract so the extension
// typechecks `require('kaminide')`. Kept in sync with
// kamin-ide/src/exthost/kaminide.d.ts. Resolved at runtime by the ext-host
// loader hook (marked external in build.mjs).
declare module "kaminide" {
  export interface KaminProject {
    readonly id: string
    readonly folderPath: string | null
    readonly createdAt: number
  }

  export interface KaminSession {
    readonly id: string
    readonly name: string
    readonly projectId: string
    readonly color?: string
    readonly pinned?: boolean
    readonly open?: boolean
    readonly lastOpened: number
    readonly createdAt: number
    readonly metadata?: Record<string, unknown>
  }

  export interface SessionsChangeEvent {
    readonly added: readonly KaminSession[]
    readonly removed: readonly KaminSession[]
    readonly changed: readonly KaminSession[]
  }

  export interface Disposable { dispose(): void }
  export type Event<T> = (listener: (e: T) => unknown, thisArgs?: unknown, disposables?: Disposable[]) => Disposable

  export interface SessionsApi {
    readonly all: readonly KaminSession[]
    readonly projects: readonly KaminProject[]
    readonly active: KaminSession | undefined
    getSession(id: string): KaminSession | undefined
    readonly onDidChangeSessions: Event<SessionsChangeEvent>
    readonly onDidChangeActiveSession: Event<KaminSession | undefined>
    readonly onDidChangeProjects: Event<readonly KaminProject[]>
    createSession(opts?: { projectId?: string; name?: string }): Promise<KaminSession>
    setActiveSession(id: string): void
    updateSession(id: string, patch: { name?: string; metadata?: Record<string, unknown> }): Promise<KaminSession | undefined>
    deleteSession(id: string): void
  }

  export const version: string
  export const sessions: SessionsApi
}
