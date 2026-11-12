// Typed fan-out of one session's normalized events. Listeners are isolated
// from each other; once closed the stream drops further events, so a late
// notification from a dying engine process cannot reach a retired consumer.
//
// Events emitted before the first subscriber are held (bounded) and handed
// to it: a session emits `session.started` and replays early engine frames
// while it is being created, before its caller can subscribe.

export class EventStream<T> {
  private readonly listeners = new Set<(event: T) => void>()
  private pending: T[] | undefined = []
  private ended = false

  constructor(
    private readonly onListenerError?: (error: unknown) => void,
    private readonly maxPending = 10_000,
  ) {}

  get closed(): boolean {
    return this.ended
  }

  subscribe(listener: (event: T) => void): () => void {
    const held = this.pending
    this.pending = undefined
    if (held) for (const event of held) this.deliver(listener, event)
    if (this.ended) return () => {}
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  emit(event: T): void {
    if (this.ended) return
    if (this.pending) {
      if (this.pending.length < this.maxPending) this.pending.push(event)
      return
    }
    for (const listener of [...this.listeners]) this.deliver(listener, event)
  }

  private deliver(listener: (event: T) => void, event: T): void {
    try {
      listener(event)
    } catch (error) {
      this.onListenerError?.(error)
    }
  }

  /** Emits `last` (if given) and releases every listener. Idempotent. */
  close(last?: T): void {
    if (this.ended) return
    if (last !== undefined) this.emit(last)
    this.ended = true
    this.listeners.clear()
  }
}
