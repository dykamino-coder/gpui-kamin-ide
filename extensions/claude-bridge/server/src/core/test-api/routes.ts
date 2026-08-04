// ============================================================================
// Test API — endpoints for testing Electron↔Server integration
// ============================================================================
//
// These endpoints simulate MCP tool calls and elicitation requests
// that would normally come from Claude CLI, routing them to the
// connected Electron client for execution.
//
// All routes are under /api/test/*
// Only available when NODE_ENV !== 'production'

import { Hono } from 'hono'
import { getAllSessions, getSession, getSessionTree, sendMcpCall } from '../pty/session-manager'
import { getSessionWs } from '../pty/session-ws'
import { createToken, listTokens } from '../auth/tokens'

export function createTestRoutes(): Hono {
  const api = new Hono()

  // ── Ping — check test API is available ────────────────
  api.get('/api/test/ping', (c) => {
    return c.json({
      ok: true,
      timestamp: new Date().toISOString(),
      sessionCount: getAllSessions().length,
    })
  })

  // ── Ensure test token exists ─────────────────────────
  // Returns existing token or creates a new one named "e2e-test"
  api.post('/api/test/ensure-token', async (c) => {
    const body = await c.req.json<{ name?: string }>().catch(() => ({} as { name?: string }))
    const name = body.name || 'e2e-test'
    const existing = (await listTokens()).find(t => t.name === name)
    if (existing) {
      return c.json({ token: existing.token, id: existing.id, created: false })
    }
    const newToken = await createToken(name)
    return c.json({ token: newToken.token, id: newToken.id, created: true })
  })

  // ── List tokens (for debugging) ────────────────────
  api.get('/api/test/tokens', async (c) => {
    return c.json({ tokens: await listTokens() })
  })

  // ── List active sessions ──────────────────────────────
  api.get('/api/test/sessions', (c) => {
    const sessions = getAllSessions()
    const list = sessions.map(s => ({
      id: s.id,
      user: s.userName,
      cwd: s.cwd,
      state: s.state,
      mcpInitialized: s.mcpInitialized,
      mcpCallCount: s.mcpCallCount,
      createdAt: s.createdAt.toISOString(),
      lastActivityAt: s.lastActivityAt.toISOString(),
      hasWs: !!getSessionWs(s.id),
      isSubAgent: s.isSubAgent,
      childSessions: s.childSessions,
      registeredToolCount: s.registeredTools.length,
      pendingElicitations: s.pendingElicitations.size,
    }))
    return c.json({ sessions: list, count: list.length })
  })

  // ── Get single session details ────────────────────────
  api.get('/api/test/sessions/:id', (c) => {
    const session = getSession(c.req.param('id'))
    if (!session) return c.json({ error: 'Session not found' }, 404)
    return c.json({
      id: session.id,
      user: session.userName,
      cwd: session.cwd,
      state: session.state,
      mcpInitialized: session.mcpInitialized,
      mcpCallCount: session.mcpCallCount,
      mcpLog: session.mcpLog.slice(-10),
      mcpLastError: session.mcpLastError,
      createdAt: session.createdAt.toISOString(),
      lastActivityAt: session.lastActivityAt.toISOString(),
      isSubAgent: session.isSubAgent,
      childSessions: session.childSessions,
      pendingElicitations: Array.from(session.pendingElicitations.keys()),
    })
  })

  // ── Session tree ──────────────────────────────────────
  api.get('/api/test/tree/:tokenId', (c) => {
    const tree = getSessionTree(c.req.param('tokenId'))
    return c.json({ tree })
  })

  // ── Simulate MCP tool call to Electron ────────────────
  // Uses the existing sendMcpCall() which creates a pending call,
  // sends it to Electron via WS, and resolves when Electron responds.
  api.post('/api/test/mcp-call/:sessionId', async (c) => {
    const sessionId = c.req.param('sessionId')
    const session = getSession(sessionId)
    if (!session) return c.json({ error: 'Session not found' }, 404)

    const ws = getSessionWs(sessionId)
    if (!ws || ws.readyState !== 1) {
      return c.json({ error: 'No WebSocket connection for session' }, 400)
    }

    const body = await c.req.json<{ toolName: string; input?: Record<string, unknown> }>()
    if (!body.toolName) return c.json({ error: 'toolName is required' }, 400)

    try {
      const result = await sendMcpCall(sessionId, body.toolName, body.input || {})
      return c.json({ result })
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      return c.json({ error: msg }, 500)
    }
  })

  // ── Simulate elicitation request to Electron ──────────
  // Sends an elicitation:request to the connected Electron client
  // and waits for the user to respond via the widget.
  api.post('/api/test/elicitation/:sessionId', async (c) => {
    const sessionId = c.req.param('sessionId')
    const session = getSession(sessionId)
    if (!session) return c.json({ error: 'Session not found' }, 404)

    const ws = getSessionWs(sessionId)
    if (!ws || ws.readyState !== 1) {
      return c.json({ error: 'No WebSocket connection for session' }, 400)
    }

    const body = await c.req.json<{ message: string; requestedSchema?: Record<string, unknown> }>()
    if (!body.message) return c.json({ error: 'message is required' }, 400)

    const requestId = `test-elicit-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`

    try {
      const result = await new Promise<{ action: string; content?: Record<string, unknown> }>((resolve, reject) => {
        const timeout = setTimeout(() => {
          session.pendingElicitations.delete(requestId)
          reject(new Error('Elicitation timed out (60s)'))
        }, 60_000)

        session.pendingElicitations.set(requestId, {
          resolve: (elicitResult) => {
            clearTimeout(timeout)
            resolve(elicitResult)
          },
          reject: (reason) => {
            clearTimeout(timeout)
            reject(reason)
          },
          toolName: 'test-elicitation',
          message: body.message,
          requestedSchema: body.requestedSchema,
          createdAt: Date.now(),
          jsonRpcId: undefined,
        })

        ws.send(JSON.stringify({
          type: 'elicitation:request',
          requestId,
          toolName: 'test-elicitation',
          message: body.message,
          requestedSchema: body.requestedSchema,
        }))
      })

      return c.json({ requestId, result })
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      return c.json({ requestId, error: msg }, 500)
    }
  })

  // ── Send raw WS message to Electron ───────────────────
  api.post('/api/test/ws-send/:sessionId', async (c) => {
    const sessionId = c.req.param('sessionId')
    const ws = getSessionWs(sessionId)
    if (!ws || ws.readyState !== 1) {
      return c.json({ error: 'No WebSocket connection for session' }, 400)
    }

    const body = await c.req.json()
    ws.send(JSON.stringify(body))
    return c.json({ sent: true })
  })

  return api
}
