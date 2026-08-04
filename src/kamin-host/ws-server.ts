// WebSocket RPC server — the renderer's DIRECT line to kamin-host
// (R1c). Same JSON frames as the parentPort link; each authenticated
// client gets its own RpcEndpoint wired to the shared method table.
// Loopback only, random port, token in the connect URL — the shell
// hands {port, token} to the renderer over the preload bridge, nothing
// is discoverable from outside the machine.
//
// This is the transport the R2 Rust shell inherits unchanged: the
// renderer never learns which shell spawned the host.
import { randomBytes } from "node:crypto"
import { createServer, type Server } from "node:http"
import { WebSocketServer, type WebSocket } from "ws"
import type { MessagePortLike } from "./port.js"
import { SLOW_FRAME_MS, isRpcFrame } from "./protocol.js"
import { RpcEndpoint } from "./rpc.js"

export type MethodTable = ReadonlyMap<string, (...params: unknown[]) => unknown>

export interface WsRpcServer {
  port: number
  token: string
  /** Fire-and-forget event to every connected client. */
  broadcast(channel: string, payload: unknown): void
  /** Request/response toward the renderer (host→renderer, B5b-2b). Targets the
   *  most-recently connected client — the single app window in practice.
   *  Rejects if no renderer is attached. */
  request<T>(method: string, ...params: unknown[]): Promise<T>
  close(): void
}

const TOKEN_BYTES = 24

function portFromSocket(ws: WebSocket): MessagePortLike {
  return {
    post: (frame) => {
      // Замер отправки: сериализация мегабайтных кадров (HTML вью) — главный
      // подозреваемый в полусекундных остановках цикла родителя.
      const t0 = Date.now()
      const text = JSON.stringify(frame)
      const tSer = Date.now()
      ws.send(text)
      const dt = Date.now() - t0
      if (dt >= SLOW_FRAME_MS) {
        const kind = (frame as { method?: string; id?: unknown }).method ?? "res"
        const KB = 1024
        console.error(`[ws send] ${kind} ${(text.length / KB) | 0}KB сериализация ${tSer - t0}ms всего ${dt}ms`)
      }
    },
    onFrame: (fn) => {
      ws.on("message", (raw: Buffer | string) => {
        let parsed: unknown
        try { parsed = JSON.parse(typeof raw === "string" ? raw : raw.toString("utf8")) } catch { return }
        if (isRpcFrame(parsed)) fn(parsed)
      })
    },
  }
}

export function startWsRpcServer(methods: MethodTable): Promise<WsRpcServer> {
  const token = randomBytes(TOKEN_BYTES).toString("base64url")
  const httpServer: Server = createServer()
  const wss = new WebSocketServer({ noServer: true })
  // Insertion-ordered: the last entry is the most-recently connected window,
  // which `request()` targets for host→renderer calls.
  const clients = new Set<RpcEndpoint>()

  httpServer.on("upgrade", (req, socket, head) => {
    const url = new URL(req.url ?? "/", "http://localhost")
    if (url.pathname !== "/rpc" || url.searchParams.get("token") !== token) {
      socket.destroy()
      return
    }
    wss.handleUpgrade(req, socket, head, (ws) => {
      const endpoint = new RpcEndpoint(portFromSocket(ws))
      // Диагностика зависаний RPC: логируем каждый вызов дольше секунды —
      // без этого «панелей нет минуту» неотличимо от «хост не отвечает».
      const SLOW_MS = 1000
      for (const [method, fn] of methods) {
        endpoint.handle(method, async (...args: unknown[]) => {
          const t0 = Date.now()
          try {
            return await (fn)(...args)
          } finally {
            const dt = Date.now() - t0
            if (dt >= SLOW_MS) console.error(`[rpc slow] ${method} ${dt}ms`)
          }
        })
      }
      clients.add(endpoint)
      ws.on("close", () => {
        clients.delete(endpoint)
        // Reject this endpoint's in-flight host→renderer calls — otherwise a
        // window reload/reconnect (frequent here) leaves every pending
        // showQuickPick / TextEditor.edit / secrets.store awaiting forever,
        // deadlocking the calling extension and leaking the pending entry.
        endpoint.failAll("renderer disconnected")
      })
    })
  })

  return new Promise<WsRpcServer>((resolve, reject) => {
    httpServer.once("error", reject)
    httpServer.listen(0, "127.0.0.1", () => {
      const addr = httpServer.address()
      if (addr === null || typeof addr === "string") {
        reject(new Error("ws-server: no bound address"))
        return
      }
      resolve({
        port: addr.port,
        token,
        broadcast: (channel, payload) => {
          for (const c of clients) c.emit(channel, payload)
        },
        request: <T>(method: string, ...params: unknown[]): Promise<T> => {
          const target = [...clients].at(-1)
          if (!target) return Promise.reject(new Error("ws-server: no renderer attached"))
          return target.call<T>(method, ...params)
        },
        close: () => { wss.close(); httpServer.close() },
      })
    })
  })
}
