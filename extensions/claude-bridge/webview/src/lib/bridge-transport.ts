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
// Diagnostic document identity only; not an authentication or cancellation key.
const generation: string = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`
const recentlySettled = new Map<number, string>()
function diagnostic(stage: string, id = 0, channel = '', documentGeneration = generation): void {
  const safeId = Number.isSafeInteger(id) && id >= 0 ? id : 0
  const safeChannel = typeof channel === 'string' && channel.length <= 256 ? channel : ''
  const safeGeneration = typeof documentGeneration === 'string' && documentGeneration.length <= 256 ? documentGeneration : ''
  try { vscodeApi.postMessage({ kind: 'invoke-diagnostic', stage, id: safeId, channel: safeChannel, generation: safeGeneration }) }
  catch { /* best-effort diagnostics cannot change invoke outcomes */ }
}
window.addEventListener('pagehide', () => diagnostic('renderer-ended'))
const pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: unknown) => void; channel: string }>()
const subs = new Map<string, Set<EventCb>>()

interface ReplyFrame { kind: "invoke-reply"; id: number; generation?: string; ok: boolean; result?: unknown; error?: unknown }
interface EventFrame { kind: "event"; channel: string; args?: unknown[] }

window.addEventListener("message", (e: MessageEvent) => {
  const msg = e.data as ReplyFrame | EventFrame | null
  if (!msg || typeof msg !== "object") return
  if (msg.kind === "invoke-reply") {
    const p = pending.get(msg.id)
    const otherDocument = typeof msg.generation === 'string' && msg.generation !== generation
    diagnostic(otherDocument ? 'renderer-other-document' : p ? 'renderer-received' : recentlySettled.has(msg.id) ? 'renderer-duplicate' : 'renderer-unknown',
      msg.id, p?.channel ?? recentlySettled.get(msg.id) ?? '', msg.generation ?? generation)
    if (!p) return
    pending.delete(msg.id)
    recentlySettled.set(msg.id, p.channel)
    if (recentlySettled.size > 64) recentlySettled.delete(recentlySettled.keys().next().value!)
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
    pending.set(id, { resolve, reject, channel: channel.length <= 256 ? channel : '' })
    try {
      vscodeApi.postMessage({ kind: "invoke", id, channel, args, generation })
      diagnostic('renderer-sent', id, channel)
    } catch (error) {
      diagnostic('renderer-send-threw', id, channel)
      throw error
    }
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
