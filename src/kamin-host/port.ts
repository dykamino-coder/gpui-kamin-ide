// Transport abstraction shared by the Node kamin-host processes. The native
// Rust/GPUI shell talks to the kamin-host parent over stdio; that parent forks
// the extension-host child over Node IPC (`process.send`). `parentPort` remains
// as compatibility for the retired utility-process launcher.
import { isRpcFrame, type RpcFrame } from "./protocol.js"

export interface MessagePortLike {
  post(frame: RpcFrame): void
  onFrame(fn: (frame: RpcFrame) => void): void
}

/** Legacy utility-process parent-port surface, typed without importing an
 * Electron runtime dependency. */
interface ParentPortLike {
  postMessage(message: unknown): void
  on(event: "message", listener: (e: { data: unknown }) => void): void
}

function getParentPort(): ParentPortLike | null {
  const candidate = (process as unknown as { parentPort?: ParentPortLike }).parentPort
  return candidate ?? null
}

export function openChildPort(): MessagePortLike {
  // The Rust shell spawns the kamin-host parent as plain `node.exe`, which gets
  // neither `parentPort` nor `process.send`. It signals stdio explicitly via
  // env. Frames are newline-delimited JSON
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
  throw new Error("kamin-host: no parent transport (set KAMIN_HOST_TRANSPORT=stdio or launch with an IPC parent)")
}
