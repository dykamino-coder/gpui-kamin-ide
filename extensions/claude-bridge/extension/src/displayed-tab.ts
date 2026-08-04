/** Which tab the chat is actually SHOWING, as opposed to which one is active.
 *
 *  The host repaints its session list, titlebar and tab strip the moment a
 *  session is clicked; the chat iframe keeps the previous transcript on screen
 *  until that session's replay lands. Nothing carried the difference out of the
 *  frame, so no host surface could indicate the gap — and a divergence that
 *  never closed (a switch dropped while the view was hidden, a tab strip switch
 *  racing a host one) had nothing anywhere that could detect it.
 *
 *  This is deliberately a report of FACT, not a request: the chat says what it
 *  has painted, and the host decides what to show. Making it a handshake the
 *  switch waits on would put a renderer that can stall on the critical path of
 *  every session click. */
export type DisplayedTabListener = (displayedTabId: string) => void

let displayed: string | null = null
const listeners = new Set<DisplayedTabListener>()

export function reportDisplayedTab(tabId: string): void {
  if (displayed === tabId) return // repeat report — nothing changed
  displayed = tabId
  for (const fn of listeners) fn(tabId)
}

export function getDisplayedTab(): string | null {
  return displayed
}

export function onDisplayedTabChange(fn: DisplayedTabListener): () => void {
  listeners.add(fn)
  return () => { listeners.delete(fn) }
}

/** A closed tab must not stay on record as displayed — the divergence check
 *  would then compare against a session that no longer exists and report a gap
 *  that can never close. */
export function forgetDisplayedTab(tabId: string): void {
  if (displayed === tabId) { displayed = null; for (const fn of listeners) fn('') }
}

/** The chat view itself was torn down: nothing is displaying any transcript, so
 *  no session is "showing". Without this, a chat webview disposed while its
 *  session stays connected (no matching tab:close) leaves the last-shown tab on
 *  record forever, and the gap check reports bridgeShowing:true for a view that
 *  no longer exists — the very stuck divergence this module exists to avoid. */
export function clearDisplayedTab(): void {
  if (displayed === null) return
  displayed = null
  for (const fn of listeners) fn('')
}
