/** Activate the KaminIDE session that owns a Bridge tab.
 *
 *  The hook is installed by SessionsBridge, which is the only thing that knows
 *  the tab ↔ session mapping. It used to live in the toast module because the
 *  "session finished → Open" toast was its only caller; switching tabs inside
 *  the webview's own strip needs the same thing, and reaching into a
 *  notification module for it would be an accident waiting to be reverted. */
export type ActivateSessionForTab = (tabId: string) => void

let handler: ActivateSessionForTab | null = null

export function setSessionActivateHandler(fn: ActivateSessionForTab): void {
  handler = fn
}

/** Whether a native session hook handled activation. Browser hosts use their
 *  existing chat route when no native session integration is installed. */
export function activateSessionForTab(tabId: string): boolean {
  if (!handler) return false
  handler(tabId)
  return true
}
