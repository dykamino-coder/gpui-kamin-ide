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
import { RPC_PEER_DISCONNECTED, type RpcFrame, type RpcRequest, type RpcResponse } from "./protocol.js"

/** Отмена вызова разрывом соединения с пиром. Отдельный тип, а не текст: по
 *  нему получатель отличает ожидаемую отмену жизненного цикла от настоящей
 *  ошибки расширения (BR-19). */
export class RpcPeerDisconnectedError extends Error {
  readonly code = RPC_PEER_DISCONNECTED
  constructor(reason: string) {
    super(reason)
    this.name = "RpcPeerDisconnectedError"
  }
}

/** Проверка по коду, а не по `instanceof`: ошибка пересекает границу процесса
 *  и восстанавливается на другой стороне как новый объект. */
export function isPeerDisconnected(err: unknown): boolean {
  return typeof err === "object" && err !== null
    && (err as { code?: unknown }).code === RPC_PEER_DISCONNECTED
}

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

  /** Reject every in-flight call — the peer process died.
   *
   *  Отклонение обязательно: иначе `showQuickPick`, правки редактора и
   *  передача секретов ждали бы вечно, а запись очереди утекала. Но это
   *  отмена жизненного цикла, а не сбой, поэтому тип отдельный: точка
   *  сдерживания падений в ребёнке пропускает именно его. Область отмены —
   *  этот endpoint, то есть одно поколение клиента: вызов, принадлежащий
   *  новому соединению, живёт в своей карте и отменён быть не может. */
  failAll(reason: string): void {
    for (const [, p] of this.pending) p.reject(new RpcPeerDisconnectedError(reason))
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
      // Код переносит классификацию через границу процесса: обработчик мог
      // ждать ответа от третьей стороны, чьё соединение и оборвалось.
      this.port.post(isPeerDisconnected(err)
        ? { kind: "res", id: req.id, ok: false, error: message, code: RPC_PEER_DISCONNECTED }
        : { kind: "res", id: req.id, ok: false, error: message })
    }
  }

  private dispatchResponse(res: RpcResponse): void {
    const p = this.pending.get(res.id)
    if (!p) return
    this.pending.delete(res.id)
    if (res.ok) { p.resolve(res.value); return }
    p.reject(res.code === RPC_PEER_DISCONNECTED
      ? new RpcPeerDisconnectedError(res.error ?? "rpc: peer disconnected")
      : new Error(res.error ?? "rpc: remote error"))
  }
}
