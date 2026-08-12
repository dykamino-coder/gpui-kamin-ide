// App-level preferences owned by KaminIDE itself (not any extension).
//
// These used to live in the Claude Bridge extension's config + UI, but they're
// really host concerns — the OS background-notification behaviour and the local
// node-pty ConPTY choice — so they moved to the native Settings page. Backed by
// a debounced JsonStore under the data dir.
import { JsonStore } from "../json-store.js"

export interface AppPrefs {
  /** Raise a native, always-on-top toast when KaminIDE is unfocused/minimised
   *  and an extension posts a message (e.g. Claude waiting / session finished). */
  backgroundToasts: boolean
  /** Use the Windows-signed system ConPTY instead of node-pty's bundled one.
   *  Off by default (bundled is instant on desktop Win11); on for corp AppLocker
   *  setups that block the unsigned OpenConsole.exe. */
  useConptyDll: boolean
  /** Drop the confirmation dialog when a session or a whole project folder is
   *  deleted from the sessions sidebar. Off by default — deleting a project
   *  takes its sessions with it and nothing on the host restores them. File
   *  tree deletions are not covered: those ask regardless (they go to the
   *  recycle bin, but the prompt is the only undo hint the user gets). */
  skipDeleteConfirm: boolean
}

const DEFAULTS: AppPrefs = {
  backgroundToasts: true,
  useConptyDll: false,
  skipDeleteConfirm: false,
}

let store: JsonStore<AppPrefs> | null = null

/** Called once at host boot. */
export function initAppPrefs(dataDir: string): void {
  store = new JsonStore(dataDir, "app-prefs", DEFAULTS)
}

export function getAppPrefs(): AppPrefs {
  return store ? store.get() : { ...DEFAULTS }
}

/** Shallow-merge a patch; returns the new full prefs. */
export function setAppPrefs(patch: Partial<AppPrefs>): AppPrefs {
  store?.set(patch)
  return getAppPrefs()
}
