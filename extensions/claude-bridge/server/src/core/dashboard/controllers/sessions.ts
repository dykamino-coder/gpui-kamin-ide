// ============================================================================
// Dashboard sessions endpoint — view active PTY sessions
// ============================================================================

import type { Hono } from 'hono'
import { getAllSessions, getSession, getSessionTree, destroySession } from '../../pty/session-manager'
import { getProcessTreeStats, startSystemStatsSampler } from '../system-stats'
import { denyApiToken, denyForeignUser, denyForeignSubject } from '../authz'

export function registerSessionRoutes(api: Hono): void {
  // GET /api/dashboard/sessions — list all active PTY sessions (admin: exposes
  // every user's cwd/title/conversationId).
  api.get('/api/dashboard/sessions', (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    startSystemStatsSampler() // lazy-start the per-process sampler
    const sessions = getAllSessions().map(s => {
      // CPU%/RSS over the session's whole process tree (PTY shell → claude node
      // → MCP children). Null on non-Linux (no /proc) or before the first sample.
      const proc = getProcessTreeStats(s.pty?.pid)
      return {
      id: s.id,
      userName: s.userName,
      cwd: s.cwd,
      state: s.state,
      createdAt: s.createdAt.toISOString(),
      lastActivityAt: s.lastActivityAt.toISOString(),
      mcpCallCount: s.mcpCallCount,
      inputCount: s.inputCount,
      // Live JSONL-derived counters for the sessions table (User/Asst/Context/
      // Total columns) — recomputed on replay, incremented on tail.
      userMessages: s.userMessages,
      assistantMessages: s.assistantMessages,
      contextTokens: s.contextTokens,
      totalTokens: s.totalTokens,
      mcpInitialized: s.mcpInitialized,
      mcpLastError: s.mcpLastError,
      mcpLog: s.mcpLog.slice(-20), // last 20 entries
      cpuPercent: proc?.cpuPercent ?? null,
      memBytes: proc?.memBytes ?? null,
      durationSec: Math.round((Date.now() - s.createdAt.getTime()) / 1000),
      idleSec: Math.round((Date.now() - s.lastActivityAt.getTime()) / 1000),
      // Client presence: the CLI/session survives a client WS drop (detach-grace
      // reattach). `attached:false` means no client is bound — the session is
      // just waiting out the grace window before the reaper kills it.
      attached: !s.detachedAt,
      detachedForSec: s.detachedAt ? Math.round((Date.now() - s.detachedAt.getTime()) / 1000) : null,
      }
    })
    return c.json({ sessions })
  })

  // POST /api/dashboard/sessions/:id/kill — kill (terminate) a running PTY
  // session from the dashboard's sessions table. destroySession() kills the
  // claude CLI process + PTY and broadcasts session:destroyed, so the table row
  // drops on the next WS event. Admin action — the dashboard sees all users'
  // sessions. NB: a distinct /kill verb, NOT `DELETE /sessions/:id` — that path
  // is already taken by the stats controller's soft-delete-session-DATA route
  // (removes JSONL/stats, does not stop the process), which would shadow this.
  api.post('/api/dashboard/sessions/:id/kill', (c) => {
    const id = c.req.param('id')
    const session = getSession(id)
    // Guard BEFORE the 404 so an api token can't distinguish "not yours" (403)
    // from "doesn't exist" (404) — both are 403 for a scoped caller.
    const denied = denyForeignUser(c, session?.userName); if (denied) return denied
    if (!session) return c.json({ error: 'Session not found' }, 404)
    destroySession(id)
    return c.json({ ok: true, id })
  })

  // GET /api/dashboard/sessions/:id/mcp-log — MCP request log for a specific session
  api.get('/api/dashboard/sessions/:id/mcp-log', (c) => {
    const session = getSession(c.req.param('id'))
    const denied = denyForeignUser(c, session?.userName); if (denied) return denied
    if (!session) return c.json({ error: 'Session not found' }, 404)
    return c.json({
      sessionId: session.id,
      mcpInitialized: session.mcpInitialized,
      mcpLastError: session.mcpLastError,
      mcpCallCount: session.mcpCallCount,
      log: session.mcpLog,
    })
  })

  // GET /api/dashboard/sessions/tree — session tree for a token (agent hierarchy)
  api.get('/api/dashboard/sessions/tree', (c) => {
    const tokenId = c.req.query('tokenId')
    if (!tokenId) return c.json({ error: 'tokenId query parameter is required' }, 400)
    const denied = denyForeignSubject(c, tokenId); if (denied) return denied
    const tree = getSessionTree(tokenId)
    return c.json({ tree })
  })

  // GET /api/dashboard/sessions/stats — session statistics (admin: aggregates
  // across all users).
  api.get('/api/dashboard/sessions/stats', (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const all = getAllSessions()
    const active = all.filter(s => s.state === 'running')
    return c.json({
      total: all.length,
      active: active.length,
      byUser: Object.fromEntries(
        [...new Set(all.map(s => s.userName))].map(u => [
          u,
          all.filter(s => s.userName === u && s.state === 'running').length,
        ])
      ),
    })
  })
}
