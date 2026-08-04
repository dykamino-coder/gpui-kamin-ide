// Message-port abstraction for the kamin-host CHILD side. The host code
// never imports `electron` — it runs either inside an Electron
// `utilityProcess` (parentPort transport) or as a plain Node child /
// Rust-spawned node.exe (process.send transport). Detection is runtime:
// utilityProcess children get `process.parentPort`, forked Node children
// get `process.send`.
import { isRpcFrame, type RpcFrame } from "./protocol.js"

export interface MessagePortLike {
  post(frame: RpcFrame): void
  onFrame(fn: (frame: RpcFrame) => void): void
}

/** Electron's utilityProcess child surface — typed locally so this
 *  module compiles without the `electron` package in scope. */
interface ParentPortLike {
  postMessage(message: unknown): void
  on(event: "message", listener: (e: { data: unknown }) => void): void
}

function getParentPort(): ParentPortLike | null {
  const candidate = (process as unknown as { parentPort?: ParentPortLike }).parentPort
  return candidate ?? null
}

export function openChildPort(): MessagePortLike {
  // R2: the Rust shell spawns a plain `node.exe` which gets NEITHER
  // `parentPort` (utilityProcess) NOR `process.send` (fork IPC). It
  // signals stdio transport explicitly via env so this branch is never
  // taken by accident under Electron. Frames are newline-delimited JSON
  // on stdin/stdout; the reader skips any non-frame line, so interleaved
  // log output on stdout is tolerated.
  if (process.env.KAMIN_HOST_TRANSPORT === "stdio") {
    return {
      post: (frame) => { process.stdout.write(`${JSON.stringify(frame)}\n`) },
      onFrame: (fn) => {
        let buf = ""
        process.stdin.setEncoding("utf8")
        process.stdin.on("data", (chunk: string) => {
          buf += chunk
          for (;;) {
            const nl = buf.indexOf("\n")
            if (nl < 0) break
            const line = buf.slice(0, nl)
            buf = buf.slice(nl + 1)
            if (!line.trim()) continue
            let parsed: unknown
            try { parsed = JSON.parse(line) } catch { continue } // non-JSON line on the wire → skip
            if (isRpcFrame(parsed)) fn(parsed)
          }
        })
        process.stdin.resume()
      },
    }
  }

  const parentPort = getParentPort()
  if (parentPort) {
    return {
      post: (frame) => { parentPort.postMessage(frame) },
      onFrame: (fn) => {
        parentPort.on("message", (e) => { if (isRpcFrame(e.data)) fn(e.data) })
      },
    }
  }
  if (typeof process.send === "function") {
    return {
      post: (frame) => { process.send?.(frame) },
      onFrame: (fn) => {
        process.on("message", (msg: unknown) => { if (isRpcFrame(msg)) fn(msg) })
      },
    }
  }
  throw new Error("kamin-host: no parent transport (run via utilityProcess.fork or child_process.fork)")
}
