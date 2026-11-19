// permessage-deflate policy of /ws/session (see attachSessionWebSocket).

/** Smallest frame worth permessage-deflate. Bulk history (pages, replays,
 *  top-ups) is far above it and keeps its 5-10× ratio; live records, status
 *  and control frames go out as they are. `BRIDGE_WS_COMPRESS_MIN_BYTES`
 *  overrides it; 0 deflates every frame, which is what the previous setting did
 *  (its 1 KiB threshold was inert under context takeover). */
export const DEFAULT_WS_COMPRESS_MIN_BYTES = 16 * 1024

export function wsCompressMinBytes(env: NodeJS.ProcessEnv = process.env): number {
  const raw = env.BRIDGE_WS_COMPRESS_MIN_BYTES
  if (raw === undefined || raw.trim() === '') return DEFAULT_WS_COMPRESS_MIN_BYTES
  const n = Number(raw)
  return Number.isInteger(n) && n >= 0 ? n : DEFAULT_WS_COMPRESS_MIN_BYTES
}

/** permessage-deflate settings of /ws/session. `ws` honours `threshold` only
 *  without server context takeover, so both go together. */
export function wsDeflateOptions(env: NodeJS.ProcessEnv = process.env): {
  threshold: number
  serverNoContextTakeover: boolean
  zlibDeflateOptions?: { level: number }
  concurrencyLimit?: number
} {
  const concurrency = Number(env.BRIDGE_WS_DEFLATE_CONCURRENCY)
  const level = Number(env.BRIDGE_WS_DEFLATE_LEVEL)
  return {
    threshold: wsCompressMinBytes(env),
    serverNoContextTakeover: env.BRIDGE_WS_DEFLATE_TAKEOVER !== '1',
    ...(Number.isInteger(concurrency) && concurrency > 0 ? { concurrencyLimit: concurrency } : {}),
    ...(Number.isInteger(level) && level >= 0 && level <= 9 && env.BRIDGE_WS_DEFLATE_LEVEL
      ? { zlibDeflateOptions: { level } }
      : {}),
  }
}
