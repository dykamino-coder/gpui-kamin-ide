// Request, send and event primitives over a host frame port. The same core
// serves the KaminIDE webview (VS Code-style postMessage) and browser sockets:
//   inv(channel, ...args)  → request/response by id  (was ipcRenderer.invoke)
//   snd(channel, ...args)  → fire-and-forget         (was ipcRenderer.send)
//   sub(channel, cb)       → event subscription      (was ipcRenderer.on)
// The host (BridgeHost) turns these back into ipcMain.handle/.on calls and
// pushes `{kind:'event'}` frames for every webContents.send.
import type { HostPort } from './socket'

export type Dispose = () => void
export type EventCallback = (...args: unknown[]) => void
export interface BridgeTransport {
  /** Rejects when the host replies with an error or the port disconnects; never retried. */
  inv(channel: string, ...args: unknown[]): Promise<unknown>
  snd(channel: string, ...args: unknown[]): void
  sub(channel: string, cb: EventCallback): Dispose
  /** Diagnostic document identity only; not an authentication or cancellation key. */
  readonly generation: string
}
type Listener = Pick<Window, 'addEventListener'>

interface ReplyFrame { kind: 'invoke-reply'; id: number; generation?: string; ok: boolean; result?: unknown; error?: unknown }
interface EventFrame { kind: 'event'; channel: string; args?: unknown[] }

/** `window` receives window-message frames when the port has no private
 * subscription, and `pagehide` diagnostics. */
export function createBridgeTransport(port: HostPort, window?: Listener): BridgeTransport {
  let seq = 0
  const generation: string = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`
  const recentlySettled = new Map<number, string>()
  function diagnostic(stage: string, id = 0, channel = '', documentGeneration = generation): void {
    const safeId = Number.isSafeInteger(id) && id >= 0 ? id : 0
    const safeChannel = typeof channel === 'string' && channel.length <= 256 ? channel : ''
    const safeGeneration = typeof documentGeneration === 'string' && documentGeneration.length <= 256 ? documentGeneration : ''
    try { port.postMessage({ kind: 'invoke-diagnostic', stage, id: safeId, channel: safeChannel, generation: safeGeneration }) }
    catch { /* best-effort diagnostics cannot change invoke outcomes */ }
  }
  window?.addEventListener('pagehide', () => diagnostic('renderer-ended'))
  const pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: unknown) => void; channel: string }>()
  const subs = new Map<string, Set<EventCallback>>()

  function receive(frame: unknown): void {
    const msg = frame as ReplyFrame | EventFrame | null
    if (!msg || typeof msg !== 'object') return
    if (msg.kind === 'invoke-reply') {
      const p = pending.get(msg.id)
      const otherDocument = typeof msg.generation === 'string' && msg.generation !== generation
      diagnostic(otherDocument ? 'renderer-other-document' : p ? 'renderer-received' : recentlySettled.has(msg.id) ? 'renderer-duplicate' : 'renderer-unknown',
        msg.id, p?.channel ?? recentlySettled.get(msg.id) ?? '', msg.generation ?? generation)
      if (port.onHostMessage && otherDocument) return
      if (!p) return
      pending.delete(msg.id)
      recentlySettled.set(msg.id, p.channel)
      if (recentlySettled.size > 64) recentlySettled.delete(recentlySettled.keys().next().value!)
      if (msg.ok) p.resolve(msg.result)
      else p.reject(new Error(String(msg.error ?? 'bridge invoke failed')))
    } else if (msg.kind === 'event') {
      const set = subs.get(msg.channel)
      if (!set) return
      const args = Array.isArray(msg.args) ? msg.args : []
      for (const cb of [...set]) {
        try { cb(...args) } catch (err) { console.error(`[bridge] handler for ${msg.channel} threw`, err) }
      }
    }
  }

  if (port.onHostMessage) port.onHostMessage(receive)
  else window?.addEventListener('message', (event: Event) => receive((event as MessageEvent).data))
  port.onDisconnect?.(() => {
    // The host may already have executed a mutation. Reject without replaying it
    // or implying cancellation; reconnect only restores read snapshots/events.
    for (const call of pending.values()) call.reject(new Error('Workspace disconnected; operation outcome may be unknown'))
    pending.clear()
  })

  return {
    generation,
    inv(channel, ...args) {
      const id = ++seq
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject, channel: channel.length <= 256 ? channel : '' })
        try {
          port.postMessage({ kind: 'invoke', id, channel, args, generation })
          diagnostic('renderer-sent', id, channel)
        } catch (error) {
          pending.delete(id)
          diagnostic('renderer-send-threw', id, channel)
          reject(error)
        }
      })
    },
    snd(channel, ...args) {
      port.postMessage({ kind: 'send', channel, args })
    },
    sub(channel, cb) {
      let set = subs.get(channel)
      if (!set) { set = new Set(); subs.set(channel, set) }
      set.add(cb)
      return () => { set?.delete(cb) }
    },
  }
}
