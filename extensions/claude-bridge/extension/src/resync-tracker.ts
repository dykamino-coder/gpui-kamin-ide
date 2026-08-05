// Resync-on-reveal bookkeeping, per webview view.
//
// A hidden webview view refuses postMessage (VS Code semantics, and our host is
// faithful to it even with retainContextWhenHidden), so every session frame the
// fan-out skips for a hidden view is a REAL gap in what that view has rendered.
// This tracks the gap where it actually happens — at the dropped send — instead
// of inferring it from a working-state transition, which missed the very common
// case of a panel hidden MID-turn (visible when the turn started, collapsed
// while the CLI was still generating: nothing re-evaluated visibility, so the
// reveal never resynced and the panel stayed a turn behind forever).
//
// Generic over the view handle so it stays free of vscode/host-compat imports
// tree and is unit-testable on its own.
export class ResyncTracker<V> {
  private readonly visible = new Map<V, boolean>()
  private readonly stale = new Set<V>()

  /** Absent = untracked (we never withhold from it, e.g. the customize views,
   *  which consume no session data). Explicit `false` = hidden right now. */
  isHidden(view: V): boolean {
    return this.visible.get(view) === false
  }

  /** Record that a resyncable frame was withheld from this view. */
  markStale(view: V): void {
    this.stale.add(view)
  }

  /** Report a visibility change. Returns true when the caller should run the
   *  resync: this view was revealed AND has a recorded gap.
   *
   *  The flag is per view and cleared only for the view being revealed. Any
   *  other view still hidden was equally missed by the resync broadcast, so it
   *  keeps its own flag and catches up on its own reveal — a single shared flag
   *  let whichever panel opened first consume it for everyone, starving the
   *  rest. (A view that is already visible is never stale: its own reveal
   *  cleared it, and the fan-out only marks views it withholds from.) */
  setVisible(view: V, isVisible: boolean): boolean {
    this.visible.set(view, isVisible)
    if (!isVisible || !this.stale.has(view)) return false
    this.stale.delete(view)
    return true
  }

  /** Drop a torn-down view. Without this, a view disposed while hidden stays in
   *  the map as "hidden" forever, so it keeps accruing staleness for a view that
   *  no longer exists. */
  forget(view: V): void {
    this.visible.delete(view)
    this.stale.delete(view)
  }

  /** Test/diagnostic accessor. */
  isStale(view: V): boolean {
    return this.stale.has(view)
  }
}
