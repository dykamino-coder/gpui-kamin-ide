// Symmetric JSON-RPC endpoint over a MessagePortLike. Used on BOTH
// sides of the shell ↔ kamin-host boundary. The native shell uses stdio; the
// extension-host child uses Node IPC behind the same transport interface.
//
// No call timeouts by design: host→shell calls can wait on the user
// (sticky toast buttons, input boxes — same convention as Bridge's
// interactive MCP tools), and shell→host calls can run long extension
// commands. Liveness is handled at the process level (exit listener +
// restart in the shell), not per-call.
import type { MessagePortLike } from "./port.js"
import type { RpcFrame, RpcRequest, RpcResponse } from "./protocol.js"

type Handler = (...params: unknown[]) => unknown
type EventListener = (channel: string, payload: unknown) => void

export class RpcEndpoint {
  private nextId = 1
  private readonly pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>()
  private readonly handlers = new Map<string, Handler>()
  private readonly eventListeners = new Set<EventListener>()

  constructor(private readonly port: MessagePortLike) {
    port.onFrame((frame) => { this.dispatch(frame) })
  }

  call<T>(method: string, ...params: unknown[]): Promise<T> {
    const id = this.nextId++
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: (v) => { resolve(v as T) }, reject })
      this.port.post({ kind: "req", id, method, params })
    })
  }

  handle(method: string, fn: Handler): void {
    this.handlers.set(method, fn)
  }

  emit(channel: string, payload: unknown): void {
    this.port.post({ kind: "evt", channel, payload })
  }

  onEvent(fn: EventListener): () => void {
    this.eventListeners.add(fn)
    return () => { this.eventListeners.delete(fn) }
  }

  /** Reject every in-flight call — the peer process died. */
  failAll(reason: string): void {
    for (const [, p] of this.pending) p.reject(new Error(reason))
    this.pending.clear()
  }

  private dispatch(frame: RpcFrame): void {
    if (frame.kind === "req") { void this.dispatchRequest(frame); return }
    if (frame.kind === "res") { this.dispatchResponse(frame); return }
    for (const fn of this.eventListeners) fn(frame.channel, frame.payload)
  }

  private async dispatchRequest(req: RpcRequest): Promise<void> {
    const handler = this.handlers.get(req.method)
    if (!handler) {
      this.port.post({ kind: "res", id: req.id, ok: false, error: `unknown method: ${req.method}` })
      return
    }
    try {
      const value = await handler(...req.params)
      this.port.post({ kind: "res", id: req.id, ok: true, value })
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      this.port.post({ kind: "res", id: req.id, ok: false, error: message })
    }
  }

  private dispatchResponse(res: RpcResponse): void {
    const p = this.pending.get(res.id)
    if (!p) return
    this.pending.delete(res.id)
    if (res.ok) p.resolve(res.value)
    else p.reject(new Error(res.error ?? "rpc: remote error"))
  }
}
