// ============================================================================
// API Token Management — CRUD + fast resolve for Bearer auth
// ============================================================================

import { randomUUID } from 'node:crypto'
import { getDb } from '../stats/database/lifecycle'

export interface ApiToken {
  id: string
  name: string
  token: string
  createdAt: string
}

export interface ResolvedToken {
  tokenId: string
  userName: string
}

// In-memory cache: bearer token → { tokenId, userName }
// Populated lazily via async DB read; subsequent CRUD invalidates so the next
// caller re-hydrates from the DB.
let tokenCache: Map<string, ResolvedToken> | null = null

async function ensureCache(): Promise<Map<string, ResolvedToken>> {
  if (tokenCache) return tokenCache
  const d = await getDb()
  const reader = await d.runAndReadAll('SELECT id, token, name FROM api_tokens')
  const rows = reader.getRowObjects() as any[]
  const next = new Map<string, ResolvedToken>()
  for (const row of rows) {
    next.set(row.token, { tokenId: row.id, userName: row.name })
  }
  tokenCache = next
  return tokenCache
}

function invalidateCache(): void {
  tokenCache = null
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

export async function createToken(name: string): Promise<ApiToken> {
  const id = randomUUID()
  const token = randomUUID()
  const createdAt = new Date().toISOString()
  // plugin_id kept as 'user' for backward compatibility with existing DB schema
  const d = await getDb()
  await d.run(
    'INSERT INTO api_tokens (id, name, plugin_id, token, created_at, cache) VALUES (?, ?, ?, ?, ?, ?)',
    [id, name, 'user', token, createdAt, 1]
  )
  invalidateCache()
  return { id, name, token, createdAt }
}

export async function deleteToken(id: string): Promise<boolean> {
  const d = await getDb()
  const countReader = await d.runAndReadAll(
    'SELECT COUNT(*) AS n FROM api_tokens WHERE id = ?',
    [id]
  )
  const n = Number((countReader.getRowObjects()[0] as { n: bigint | number }).n)
  if (n === 0) {
    invalidateCache()
    return false
  }
  await d.run('DELETE FROM api_tokens WHERE id = ?', [id])
  invalidateCache()
  return true
}

export async function listTokens(): Promise<ApiToken[]> {
  const d = await getDb()
  const reader = await d.runAndReadAll(
    'SELECT id, name, token, created_at FROM api_tokens ORDER BY created_at DESC'
  )
  const rows = reader.getRowObjects() as any[]
  return rows.map(r => ({
    id: r.id,
    name: r.name,
    token: r.token,
    createdAt: r.created_at,
  }))
}

/** Mask a token value for responses that must not leak the raw secret to a
 *  non-admin caller (`abcd…wxyz`). Keeps enough to recognise the token you
 *  already hold without handing an attacker a usable credential. */
export function maskToken(token: string): string {
  if (!token || token.length <= 8) return '••••'
  return `${token.slice(0, 4)}…${token.slice(-4)}`
}

/** Resolve a caller-presented token value → its own {id, name, createdAt}.
 *  Proof-of-possession: you must already hold the token, so returning its own
 *  metadata is not a leak. Used by clients that previously listed ALL tokens
 *  and matched by value (which exposed every user's raw secret). */
export async function findTokenByValue(
  token: string,
): Promise<Pick<ApiToken, 'id' | 'name' | 'createdAt'> | null> {
  if (!token) return null
  const d = await getDb()
  const reader = await d.runAndReadAll(
    'SELECT id, name, created_at FROM api_tokens WHERE token = ?',
    [token]
  )
  const rows = reader.getRowObjects() as any[]
  if (!rows.length) return null
  return { id: rows[0].id, name: rows[0].name, createdAt: rows[0].created_at }
}

export async function updateToken(id: string, patch: { name?: string }): Promise<ApiToken | null> {
  const d = await getDb()
  const existingReader = await d.runAndReadAll(
    'SELECT id FROM api_tokens WHERE id = ?',
    [id]
  )
  const existing = existingReader.getRowObjects()[0]
  if (!existing) return null

  if (patch.name !== undefined) {
    await d.run('UPDATE api_tokens SET name = ? WHERE id = ?', [patch.name, id])
  }

  invalidateCache()

  const reader = await d.runAndReadAll(
    'SELECT id, name, token, created_at FROM api_tokens WHERE id = ?',
    [id]
  )
  const row = reader.getRowObjects()[0] as any
  return {
    id: row.id,
    name: row.name,
    token: row.token,
    createdAt: row.created_at,
  }
}

// ---------------------------------------------------------------------------
// Resolve — hot path, uses cache
// ---------------------------------------------------------------------------

/** Async resolver — hydrates cache on first call. Use this for new callsites. */
export async function resolveToken(bearer: string): Promise<ResolvedToken | null> {
  const cache = await ensureCache()
  return cache.get(bearer) ?? null
}

/** Sync resolver for hot paths — only valid AFTER `await primeTokenCache()` has
 *  resolved at server boot. Returns null until the cache is hydrated. */
export function resolveTokenSync(bearer: string): ResolvedToken | null {
  if (!tokenCache) return null
  return tokenCache.get(bearer) ?? null
}

/** Eagerly hydrate the in-memory token cache. Call once at server startup so
 *  `resolveTokenSync` works without spamming `await` on every Bearer check. */
export async function primeTokenCache(): Promise<void> {
  await ensureCache()
}
