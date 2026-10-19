// Invoke replies a hidden webview view refused, kept until it is shown again.
//
// A webview view that is hidden refuses `postMessage` (VS Code semantics, which
// our host reproduces). Events lost that way are repaired by resync-on-reveal
// (`ResyncTracker`), but an `invoke-reply` is not an event: it answers one
// request of a page that is alive and waiting. KaminIDE creates the Console
// view hidden and its page runs its start-up requests invisibly; every reply
// the host refused left `init()` waiting forever behind "Restoring your
// session…" (INC-2026-0065). The reply is therefore held per view and sent
// when the view becomes visible.
//
// Bounded per view: one entry per outstanding request, so the cap is only a
// guard against a page that keeps issuing requests while hidden.

export interface DeferredReply {
  /** The `invoke-reply` frame, exactly as it was first sent. */
  frame: Record<string, unknown>
  /** Reports the outcome of the later send (diagnostics only). */
  flushed: (accepted: boolean) => void
}

export const MAX_DEFERRED_REPLIES_PER_VIEW = 256

export class DeferredReplies<V extends object> {
  private readonly byView = new WeakMap<V, DeferredReply[]>()

  /** Hold a refused reply. Returns the oldest entry when the cap evicts one. */
  defer(view: V, entry: DeferredReply): DeferredReply | undefined {
    const list = this.byView.get(view) ?? []
    list.push(entry)
    this.byView.set(view, list)
    return list.length > MAX_DEFERRED_REPLIES_PER_VIEW ? list.shift() : undefined
  }

  /** Remove and return everything held for the view, oldest first. */
  take(view: V): DeferredReply[] {
    const list = this.byView.get(view) ?? []
    this.byView.delete(view)
    return list
  }

  /** Put entries a flush could not deliver back in front of newer ones. */
  restore(view: V, entries: DeferredReply[]): void {
    if (entries.length === 0) return
    const list = this.byView.get(view) ?? []
    this.byView.set(view, [...entries, ...list].slice(-MAX_DEFERRED_REPLIES_PER_VIEW))
  }

  forget(view: V): void {
    this.byView.delete(view)
  }

  size(view: V): number {
    return this.byView.get(view)?.length ?? 0
  }
}
