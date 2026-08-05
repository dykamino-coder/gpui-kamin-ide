// Coalescing relay queue for `webview.postMessage` (extracted from webview.ts).
//
// Each extension postMessage used to be its OWN host→renderer broadcast, so a
// status storm (e.g. a flapping MCP server re-broadcasting its server list)
// fired dozens of ~16KB frames in a burst → the renderer main thread choked
// JSON-parsing + dispatching them one by one and went "Not Responding" (prod
// freeze.log: lastCh=kamin:webview:post, heap fine → not OOM, a relay flood).
// Queue posts and flush ONE batched frame per event-loop turn; the renderer
// unwraps the array in a single parse+dispatch.

type Broadcast = (channel: string, payload: unknown) => void

interface QueuedPost {
  id: string
  msg: unknown
  settle: (delivered: boolean) => void
}

export class WebviewPostQueue {
  private queue: QueuedPost[] = []
  private flushTimer: ReturnType<typeof setImmediate> | null = null

  // Cap how many posts ride in ONE frame. Coalescing without a size cap only
  // trades "many small renderer tasks" for "one enormous" — the renderer has to
  // JSON.parse the frame and structured-clone each msg into the iframe on its
  // main thread, uninterruptibly. Draining the WHOLE queue would also re-merge
  // the entry chunks the extension's JsonlBatcher deliberately splits, undoing
  // that fix. Bound the frame and re-schedule the rest on the next tick.
  //
  // NOT capped by LENGTH, deliberately: this queue carries chat entries, so a
  // drop-oldest cap would re-create the "chat lost a turn" bug the batcher's
  // size cap exists to prevent — strictly worse than the memory it would save.
  // It also can't run away: the drain is one turn of the event loop per 8 posts,
  // so a producer must sustain >8 posts per LOOP TURN forever to outpace it. If
  // it's ever that saturated the renderer is already lost and dropping won't
  // save it. Bound the frame; never the backlog.
  private static readonly MAX_POSTS_PER_FLUSH = 8

  // Also cap a frame by its SERIALISED SIZE, not only its post count. A DLP agent
  // that hooks the heap free-path and blocks holding the process heap lock
  // (siph64/SearchInform — see investigation_not_responding_freeze) only wedges
  // allocations that go through the MAIN heap lock, i.e. LARGE ones; small
  // allocations use the lock-free LFH front-end and survive (REPRODUCED: 64B ran
  // through the poison, 128KB froze). Coalescing up to 8 posts could build a
  // ~128KB frame — Chromium then allocates a large receive/clone buffer for it in
  // the very process that wedges. Keeping each frame under the LFH threshold
  // (~16KB; 12KB leaves headroom for the batch envelope) keeps those allocations
  // on the lock-free path. Best-effort ONLY: a single post already larger than
  // this still goes as one frame (splitting it needs an iframe-protocol change),
  // and whether Chromium routes these through the default heap at all is
  // UNVERIFIED locally — but a smaller frame can never be worse.
  private static readonly SOFT_MAX_FRAME_KB = 12
  private static readonly BYTES_PER_KB = 1024
  private static readonly SOFT_MAX_FRAME_BYTES = WebviewPostQueue.SOFT_MAX_FRAME_KB * WebviewPostQueue.BYTES_PER_KB

  constructor(private readonly broadcast: Broadcast) {}

  /** Размеры уже посчитанных сообщений. Фан-аут (BridgeHost.post в N вью) даёт
   *  N записей на ОДНУ ссылку, и раньше каждая стрингифаилась заново; плюс тот
   *  же объект мерился повторно на каждом кадре, пока лежал в очереди. На
   *  реплей-шторме это гигабайты транзиентных строк (аудит памяти хоста). */
  private static readonly sizeCache = new WeakMap<object, number>()

  /** Cheap size proxy — `JSON.stringify(...).length` is the honest cost since the
   *  broadcast serialises the same object, and the batch is ≤8 posts so this is
   *  bounded. A msg that fails to serialise counts as 0 (it can't inflate a frame
   *  it can't be part of). Результат кэшируется по ССЫЛКЕ (WeakMap — не держит
   *  сообщение живым). */
  private static sizeOf(msg: unknown): number {
    if (msg === null || typeof msg !== 'object') {
      try {
        const serialized: unknown = JSON.stringify(msg)
        return typeof serialized === 'string' ? serialized.length : 0
      } catch { return 0 }
    }
    const cached = WebviewPostQueue.sizeCache.get(msg)
    if (cached !== undefined) return cached
    let size: number
    try {
      const serialized: unknown = JSON.stringify(msg)
      size = typeof serialized === 'string' ? serialized.length : 0
    } catch { size = 0 }
    WebviewPostQueue.sizeCache.set(msg, size)
    return size
  }

  /** Queue a post; the promise settles when the frame carrying it is actually
   *  handed to `broadcast` (d.ts: postMessage resolves "when the message is
   *  posted to a webview … or when it is dropped"), NOT at enqueue time. */
  enqueue(id: string, msg: unknown): Promise<boolean> {
    return new Promise<boolean>((resolve) => {
      this.queue.push({ id, msg, settle: resolve })
      this.schedule()
    })
  }

  private schedule(): void {
    if (this.flushTimer) return
    this.flushTimer = setImmediate(() => {
      this.flushTimer = null
      this.sendOneFrame()
      if (this.queue.length > 0) this.schedule() // more pending → next tick
    })
  }

  private sendOneFrame(): void {
    if (this.queue.length === 0) return
    // Take posts up to the count cap OR the size cap, whichever comes first —
    // but always at least one (a lone oversized post can't be split here).
    let take = 0
    let bytes = 0
    while (
      take < this.queue.length &&
      take < WebviewPostQueue.MAX_POSTS_PER_FLUSH
    ) {
      const size = WebviewPostQueue.sizeOf(this.queue[take]?.msg)
      if (take > 0 && bytes + size > WebviewPostQueue.SOFT_MAX_FRAME_BYTES) break
      bytes += size
      take++
    }
    const batch = this.queue.splice(0, take)
    if (batch.length === 0) return
    // Group by msg IDENTITY. An extension that posts to several webviews hands
    // the SAME object to each (BridgeHost.post), so a fan-out lands here as N
    // entries sharing one reference. The child→parent V8 IPC already dedups
    // those (structured clone emits back-references) — but the host→renderer
    // hop is `JSON.stringify`, which has NO reference dedup and serialises the
    // payload N times. Measured: 3 refs → 100KB via v8, 300KB via JSON; on a
    // real replay frame 46.4MB → 15.5MB and renderer parse 191ms → 47ms.
    // N ≤ MAX_POSTS_PER_FLUSH, so the linear scan is trivial.
    const groups: { ids: string[]; msg: unknown }[] = []
    for (const p of batch) {
      const g = groups.find((x) => x.msg === p.msg)
      if (g) g.ids.push(p.id)
      else groups.push({ ids: [p.id], msg: p.msg })
    }
    this.broadcast("kamin:webview:post", { batch: groups })
    for (const p of batch) p.settle(true)
  }

  /** Send everything queued NOW, in order, still respecting the frame cap. */
  private flushNow(): void {
    if (this.flushTimer) { clearImmediate(this.flushTimer); this.flushTimer = null }
    while (this.queue.length > 0) this.sendOneFrame()
  }

  /** Every webview broadcast that is NOT a queued post must go through here.
   *  WHY: posts are deferred to a setImmediate frame while html/title/reveal/
   *  dispose broadcast synchronously — so `postMessage(x); panel.dispose()`
   *  (the idiomatic "send final state, then close") delivered dispose FIRST and
   *  the renderer unmounted the iframe before the queued frame ever flushed,
   *  dropping x silently while postMessage had already resolved true. A
   *  microtask can never overtake a setImmediate, so that was deterministic,
   *  not a rare race. Draining first restores the extension's own ordering. */
  broadcastAfterPosts(channel: string, payload: unknown): void {
    this.flushNow()
    this.broadcast(channel, payload)
  }

  /** Drop posts still queued for a webview the RENDERER already closed: its
   *  iframe is gone, so they aren't deliverable — settle them false rather than
   *  broadcasting into a dead id. (An extension-initiated dispose instead goes
   *  through broadcastAfterPosts, so the message it meant to send still lands.) */
  purge(id: string): void {
    if (this.queue.length === 0) return
    this.queue = this.queue.filter((p) => {
      if (p.id !== id) return true
      p.settle(false)
      return false
    })
  }
}
