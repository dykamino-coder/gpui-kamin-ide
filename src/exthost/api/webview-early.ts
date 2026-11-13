// BR-05: after an extension-host respawn the renderer's view iframes live on and
// keep sending, but a view only exists in the new host once it is resolved
// again. Messages for a view that is not resolved yet wait here and are handed
// over in order right after its provider runs, instead of being dropped.

/** Panels exist from creation on, so an unknown panel id is a dead one. */
const PANEL_ID = /^webview-\d+$/
const MAX_VIEWS = 16
const MAX_MESSAGES_PER_VIEW = 256

export class EarlyViewMessages {
  private readonly byView = new Map<string, unknown[]>()

  /** Keep a message for a view that is not resolved yet (bounded, oldest dropped). */
  keep(id: string, msg: unknown): void {
    if (PANEL_ID.test(id)) return
    const queue = this.byView.get(id)
    if (queue) {
      if (queue.length >= MAX_MESSAGES_PER_VIEW) queue.shift()
      queue.push(msg)
    } else if (this.byView.size < MAX_VIEWS) {
      this.byView.set(id, [msg])
    }
  }

  /** Remove and return the messages kept for a view, oldest first. */
  take(id: string): unknown[] {
    const queue = this.byView.get(id) ?? []
    this.byView.delete(id)
    return queue
  }
}
