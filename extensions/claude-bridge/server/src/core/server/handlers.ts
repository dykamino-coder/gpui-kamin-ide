// ============================================================================
// Request Handlers - Health endpoint
// ============================================================================

import type { Context } from "hono"
import { VERSION, SERVICE_NAME } from "../config"
import { getAllSessions } from "../pty/session-manager"

export async function handleHealth(c: Context) {
  const sessions = getAllSessions()
  const activeSessions = sessions.filter(s => s.state === 'running')

  return c.json({
    status: "ok",
    service: SERVICE_NAME,
    version: VERSION,
    mode: "pty",
    sessions: {
      total: sessions.length,
      active: activeSessions.length,
    },
  })
}
