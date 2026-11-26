// permessage-deflate policy of /ws/session (see attachSessionWebSocket).
//
// Measured on 200 chats streaming at once (fake CLI, 20 clients x 10 chats):
// deflating every live record kept delivery p95 at 3.6-3.8 s, because `ws` runs
// at most `concurrencyLimit` deflates at a time across all sockets and each one
// waits for a busy main thread to collect it. Sending small frames raw brought
// p95 to ~0.6 s at ~6x the wire bytes of live traffic. Bulk frames (history
// pages, replays, top-ups) stay deflated in both modes.

/** Smallest frame worth permessage-deflate by default. */
export const DEFAULT_WS_COMPRESS_MIN_BYTES = 16 * 1024

/** `BRIDGE_WS_COMPRESS_MIN_BYTES`: frames below it go out raw. 0 deflates every
 *  frame (for slow links): then with context takeover, the fastest level and a
 *  wider deflate pool, which kept wire bytes at the old level (p95 ~1.9 s). */
export function wsCompressMinBytes(env: NodeJS.ProcessEnv = process.env): number {
  const raw = env.BRIDGE_WS_COMPRESS_MIN_BYTES
  if (raw === undefined || raw.trim() === '') return DEFAULT_WS_COMPRESS_MIN_BYTES
  const n = Number(raw)
  return Number.isInteger(n) && n >= 0 ? n : DEFAULT_WS_COMPRESS_MIN_BYTES
}

export interface WsDeflateOptions {
  threshold: number
  serverNoContextTakeover: boolean
  zlibDeflateOptions?: { level: number }
  concurrencyLimit?: number
}

/** permessage-deflate settings of /ws/session. `ws` honours `threshold` only
 *  without server context takeover, so a size cut-off and takeover exclude
 *  each other: takeover is used only when everything is deflated anyway. */
export function wsDeflateOptions(env: NodeJS.ProcessEnv = process.env): WsDeflateOptions {
  const threshold = wsCompressMinBytes(env)
  if (threshold > 0) return { threshold, serverNoContextTakeover: true }
  return { threshold: 0, serverNoContextTakeover: false, zlibDeflateOptions: { level: 1 }, concurrencyLimit: 64 }
}
