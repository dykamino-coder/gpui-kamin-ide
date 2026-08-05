// postMessage transport for the Bridge webview ↔ extension host.
//
// Mirrors the three legacy Electron-IPC-shaped primitives over the webview channel:
//   inv(channel, ...args)  → request/response by id  (was ipcRenderer.invoke)
//   snd(channel, ...args)  → fire-and-forget         (was ipcRenderer.send)
//   sub(channel, cb)       → event subscription      (was ipcRenderer.on)
// The host (BridgeHost) turns these back into ipcMain.handle/.on calls and
// pushes `{kind:'event'}` frames for every webContents.send.
import { vscodeApi } from "./webview-api.js"

type Disposer = () => void
type EventCb = (...args: unknown[]) => void

let seq = 0
const pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: unknown) => void }>()
const subs = new Map<string, Set<EventCb>>()

interface ReplyFrame { kind: "invoke-reply"; id: number; ok: boolean; result?: unknown; error?: unknown }
interface EventFrame { kind: "event"; channel: string; args?: unknown[] }

window.addEventListener("message", (e: MessageEvent) => {
  const msg = e.data as ReplyFrame | EventFrame | null
  if (!msg || typeof msg !== "object") return
  if (msg.kind === "invoke-reply") {
    const p = pending.get(msg.id)
    if (!p) return
    pending.delete(msg.id)
    if (msg.ok) p.resolve(msg.result)
    else p.reject(new Error(String(msg.error ?? "bridge invoke failed")))
  } else if (msg.kind === "event") {
    const set = subs.get(msg.channel)
    if (!set) return
    const args = Array.isArray(msg.args) ? msg.args : []
    for (const cb of [...set]) {
      try { cb(...args) } catch (err) { console.error(`[bridge] handler for ${msg.channel} threw`, err) }
    }
  }
})

export function inv(channel: string, ...args: unknown[]): Promise<unknown> {
  const id = ++seq
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject })
    vscodeApi.postMessage({ kind: "invoke", id, channel, args })
  })
}

export function snd(channel: string, ...args: unknown[]): void {
  vscodeApi.postMessage({ kind: "send", channel, args })
}

export function sub(channel: string, cb: EventCb): Disposer {
  let set = subs.get(channel)
  if (!set) { set = new Set(); subs.set(channel, set) }
  set.add(cb)
  return () => { set?.delete(cb) }
}
