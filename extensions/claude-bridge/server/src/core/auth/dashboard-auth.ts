// ============================================================================
// Dashboard Authentication — session management for dashboard UI
// ============================================================================

import { randomUUID } from 'crypto'
import { getDb } from '../stats/database/lifecycle'

const SESSION_TTL_MS = 24 * 60 * 60 * 1000 // 24 hours

export interface DashboardSession {
  id: string
  token: string
  createdAt: string
  expiresAt: string
}

// ---------------------------------------------------------------------------
// Environment-based credentials
// ---------------------------------------------------------------------------

export function getDashboardCredentials(): { user: string; password: string } | null {
  const user = process.env.DASHBOARD_USER
  const password = process.env.DASHBOARD_PASSWORD
  if (!user || !password) return null
  return { user, password }
}

/** Returns true if dashboard auth is enabled (env vars set) */
export function isDashboardAuthEnabled(): boolean {
  return getDashboardCredentials() !== null
}

// ---------------------------------------------------------------------------
// Session CRUD
// ---------------------------------------------------------------------------

export async function createDashboardSession(): Promise<DashboardSession> {
  const d = await getDb()
  const id = randomUUID()
  const token = randomUUID()
  const now = new Date()
  const createdAt = now.toISOString()
  const expiresAt = new Date(now.getTime() + SESSION_TTL_MS).toISOString()

  await d.run(
    'INSERT INTO dashboard_sessions (id, token, created_at, expires_at) VALUES (?, ?, ?, ?)',
    [id, token, createdAt, expiresAt]
  )

  return { id, token, createdAt, expiresAt }
}

export async function validateDashboardSession(token: string): Promise<DashboardSession | null> {
  const d = await getDb()
  const reader = await d.runAndReadAll(
    'SELECT id, token, created_at, expires_at FROM dashboard_sessions WHERE token = ?',
    [token]
  )
  const row = reader.getRowObjects()[0] as
    | { id: string; token: string; created_at: string; expires_at: string }
    | undefined

  if (!row) return null

  // Check expiry
  if (new Date(row.expires_at).getTime() < Date.now()) {
    await d.run('DELETE FROM dashboard_sessions WHERE id = ?', [row.id])
    return null
  }

  return {
    id: row.id,
    token: row.token,
    createdAt: row.created_at,
    expiresAt: row.expires_at,
  }
}

export async function deleteDashboardSession(token: string): Promise<boolean> {
  const d = await getDb()
  // DuckDB run() doesn't return row counts — count first.
  const countReader = await d.runAndReadAll(
    'SELECT COUNT(*) AS n FROM dashboard_sessions WHERE token = ?',
    [token]
  )
  const n = Number((countReader.getRowObjects()[0] as { n: bigint | number }).n)
  if (n === 0) return false
  await d.run('DELETE FROM dashboard_sessions WHERE token = ?', [token])
  return true
}

export async function cleanExpiredSessions(): Promise<number> {
  const d = await getDb()
  const nowIso = new Date().toISOString()
  const countReader = await d.runAndReadAll(
    'SELECT COUNT(*) AS n FROM dashboard_sessions WHERE expires_at < ?',
    [nowIso]
  )
  const n = Number((countReader.getRowObjects()[0] as { n: bigint | number }).n)
  if (n === 0) return 0
  await d.run('DELETE FROM dashboard_sessions WHERE expires_at < ?', [nowIso])
  return n
}

/** Validate username/password against env vars */
export function validateCredentials(username: string, password: string): boolean {
  const creds = getDashboardCredentials()
  if (!creds) return false
  return creds.user === username && creds.password === password
}
