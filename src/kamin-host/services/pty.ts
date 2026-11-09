// Native host facade; the browser host uses the same PTY implementation.
import { getAppPrefs } from "./app-prefs.js"
import { applyEnvCollections } from "./env-collection.js"
import { PtySessions, type PtyCreateOpts, type PtyEvents } from "./pty-sessions.js"

export type { PtyCreateOpts, PtyEvents } from "./pty-sessions.js"

let events: PtyEvents | null = null
const sessions = new PtySessions({
  onData: (id, data) => events?.onData(id, data),
  onExit: (id, code, signal) => events?.onExit(id, code, signal),
}, () => applyEnvCollections(process.env), () => getAppPrefs().useConptyDll)

/** Called once at host boot — data/exit fan out to the shell transport. */
export function initPtyEvents(ev: PtyEvents): void { events = ev }
export function createSession(opts: PtyCreateOpts): Promise<string> { return sessions.createSession(opts) }
export function writeToSession(id: string, data: string): void { sessions.writeToSession(id, data) }
export function resizeSession(id: string, cols: number, rows: number): void { sessions.resizeSession(id, cols, rows) }
export function disposeSession(id: string): void { sessions.disposeSession(id) }
export function disposeAllSessions(): void { sessions.disposeAllSessions() }
