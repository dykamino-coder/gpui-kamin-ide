// ============================================================================
// Per-token streaming settings — read/write through DuckDB user_token_settings.
// In-memory cache with 60s TTL. WS push notifies bridge consumers on PUT so
// new PTY-spawns pick up the latest values without restart.
// ============================================================================

import { getDb } from "../stats/database/lifecycle"

export interface StreamingSettings {
  enabled: boolean
  showSideQuests: boolean
  showThinkingStream: boolean
  captureRequestBodies: boolean
}

// Default OFF until the native MITM TLS handshake reliability is fixed —
// re-enable per token via Settings panel, or set OCB_STREAMING_DEFAULT=enabled
// on the bridge container. Earlier default was ON, but a TLS termination
// regression in 6.2.5/6.2.6 caused ECONNRESETs on api.anthropic.com for some
// users; opt-in keeps the failure mode contained.
export const DEFAULT_STREAMING_SETTINGS: StreamingSettings = {
  enabled: process.env.OCB_STREAMING_DEFAULT === "enabled",
  showSideQuests: false,
  showThinkingStream: true,
  captureRequestBodies: false,
}

const cache = new Map<string, { value: StreamingSettings; expiresAt: number }>()
const CACHE_TTL_MS = 60_000

export async function getStreamingSettings(tokenId: string): Promise<StreamingSettings> {
  const hit = cache.get(tokenId)
  if (hit && hit.expiresAt > Date.now()) return hit.value
  let row: any = null
  try {
    const db = await getDb()
    const rows = (await db.runAndReadAll(
      `SELECT streaming_enabled, show_side_quests, show_thinking_stream, capture_request_bodies
       FROM user_token_settings WHERE token_id = ?`,
      [tokenId],
    )).getRowObjects()
    row = rows[0]
  } catch { /* fall through to defaults */ }
  const value: StreamingSettings = row
    ? {
        enabled: !!row.streaming_enabled,
        showSideQuests: !!row.show_side_quests,
        showThinkingStream: !!row.show_thinking_stream,
        captureRequestBodies: !!row.capture_request_bodies,
      }
    : { ...DEFAULT_STREAMING_SETTINGS }
  cache.set(tokenId, { value, expiresAt: Date.now() + CACHE_TTL_MS })
  return value
}

export async function setStreamingSettings(tokenId: string, settings: StreamingSettings): Promise<void> {
  const db = await getDb()
  const now = new Date().toISOString()
  await db.run(
    `INSERT INTO user_token_settings
       (token_id, streaming_enabled, show_side_quests, show_thinking_stream, capture_request_bodies, updated_at)
     VALUES (?, ?, ?, ?, ?, ?)
     ON CONFLICT (token_id) DO UPDATE SET
       streaming_enabled = excluded.streaming_enabled,
       show_side_quests = excluded.show_side_quests,
       show_thinking_stream = excluded.show_thinking_stream,
       capture_request_bodies = excluded.capture_request_bodies,
       updated_at = excluded.updated_at`,
    [tokenId, settings.enabled, settings.showSideQuests, settings.showThinkingStream, settings.captureRequestBodies, now],
  )
  cache.set(tokenId, { value: { ...settings }, expiresAt: Date.now() + CACHE_TTL_MS })
}

export function invalidateStreamingSettingsCache(tokenId?: string): void {
  if (tokenId) cache.delete(tokenId)
  else cache.clear()
}
