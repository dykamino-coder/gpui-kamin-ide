// Module-scoped shared primitives. ALL extensions get the same identity
// for `Disposable`, `EventEmitter`, `Uri` — without that, two extensions
// exchanging a Disposable would fail `instanceof` checks because each
// would have its own constructor.
//
// `Uri` re-exports the canonical `vscode-uri` package — the very same
// code VS Code ships. Extensions get correct `parse`/`joinPath`/`with`
// behaviour without us re-implementing it.
import { URI, Utils } from "vscode-uri"

// `vscode.Uri.joinPath` is a STATIC on Uri, but vscode-uri keeps it on a
// separate `Utils` export — graft it on so `Uri.joinPath(base, ...segs)` works
// (jupyter et al. rely on it). Mutating URI itself (not a wrapper) keeps
// `instanceof Uri` intact across extensions.
export const Uri = Object.assign(URI, {
  joinPath: (base: URI, ...pathSegments: string[]): URI => Utils.joinPath(base, ...pathSegments),
})

export class Disposable {
  static from(...disposables: { dispose(): unknown }[]): Disposable {
    return new Disposable(() => {
      for (const d of disposables) {
        try { d.dispose() } catch (err) { console.warn("Disposable.from: dispose threw", err) }
      }
    })
  }
  constructor(private readonly _onDispose: () => unknown) {}
  dispose(): void { this._onDispose() }
}

/** d.ts §1810: `Event<T>` is `(listener, thisArgs?, disposables?): Disposable`. */
type EventListener<T> = (e: T) => void
export type EventSignature<T> = (listener: EventListener<T>, thisArgs?: unknown, disposables?: Disposable[]) => Disposable

export class EventEmitter<T> {
  private readonly listeners = new Set<EventListener<T>>()
  readonly event: EventSignature<T> = (listener, thisArgs, disposables) => {
    const bound = thisArgs !== undefined ? listener.bind(thisArgs) : listener
    this.listeners.add(bound)
    const sub = new Disposable(() => { this.listeners.delete(bound) })
    if (disposables) disposables.push(sub)
    return sub
  }
  fire(data: T): void {
    for (const l of [...this.listeners]) {
      try { l(data) } catch (err) { console.error("EventEmitter listener threw:", err) }
    }
  }
  dispose(): void { this.listeners.clear() }
}

/** Lightweight always-disposed event for things we don't track yet (e.g.
 *  `onDidChangeWindowState`). Extensions can subscribe; the listener
 *  never fires; subscriptions clean up cleanly. Replace with a real
 *  emitter when the host gains the underlying signal. */
export const noopEvent: EventSignature<never> = () => new Disposable(() => { /* no-op */ })
