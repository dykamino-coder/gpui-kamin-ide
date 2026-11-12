// Per-user (per Bridge token) "main agent": the engine new chats open with.
// Stored in DuckDB user_agent_settings, cached briefly like the streaming
// settings. Unknown or missing values mean Claude, the existing behaviour.
import type { AgentEngineId } from '../../shared/agent-events'
import { getDb } from '../stats/database/lifecycle'
import { agentEngineOrDefault, DEFAULT_AGENT_ENGINE } from './engines'

const cache = new Map<string, { value: AgentEngineId; expiresAt: number }>()
const CACHE_TTL_MS = 60_000

export async function getDefaultEngine(tokenId: string): Promise<AgentEngineId> {
  const hit = cache.get(tokenId)
  if (hit && hit.expiresAt > Date.now()) return hit.value
  let value: AgentEngineId = DEFAULT_AGENT_ENGINE
  try {
    const db = await getDb()
    const rows = (
      await db.runAndReadAll(`SELECT default_engine FROM user_agent_settings WHERE token_id = ?`, [tokenId])
    ).getRowObjects()
    value = agentEngineOrDefault(rows[0]?.default_engine)
  } catch {
    /* defaults */
  }
  cache.set(tokenId, { value, expiresAt: Date.now() + CACHE_TTL_MS })
  return value
}

export async function setDefaultEngine(tokenId: string, engine: AgentEngineId): Promise<void> {
  const db = await getDb()
  await db.run(
    `INSERT INTO user_agent_settings (token_id, default_engine, updated_at)
     VALUES (?, ?, ?)
     ON CONFLICT (token_id) DO UPDATE SET
       default_engine = excluded.default_engine,
       updated_at = excluded.updated_at`,
    [tokenId, engine, new Date().toISOString()],
  )
  cache.set(tokenId, { value: engine, expiresAt: Date.now() + CACHE_TTL_MS })
}

export function invalidateDefaultEngineCache(): void {
  cache.clear()
}
