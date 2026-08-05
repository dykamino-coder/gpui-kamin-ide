// HTTP endpoint that CLI POSTs to when a rewritten hook fires.
// URL shape: POST /api/hooks/:sessionId/:event/:hookId
// Auth: Bearer token (same as MCP token for the session).
//
// CLI's `http` hook flow:
//   1. CLI sends POST with JSON body = hook input payload
//   2. We look up the hook in the registry and dispatch (local/server/http)
//   3. We return JSON shape that CLI's hook-output protocol expects:
//        { continue: true|false, stopReason?, suppressOutput?,
//          systemMessage?, decision?, hookSpecificOutput? }
//      For now we just pass through stdout-as-text (CLI tolerates plain text);
//      structured override comes from hook's own JSON-stdout if any.

import { Hono } from 'hono'
import { dispatchHook } from './dispatcher'
import { getSession } from '../pty/session-manager'
import type { HookInputPayload } from './types'
import { warnLog } from '../logging'
import { readRecentExecutions, clearLog } from './audit-log'
import { listSession } from './registry'
import { HOOK_LIBRARY } from './library'
import { emitBridgeEvent } from './bridge-emitter'
import { applyDeterministicStatus, isBridgeStatusEvent } from './bridge-status-hooks'
import { BRIDGE_HOOK_EVENTS, type BridgeHookEvent } from './types'
import { resolveToken } from '../auth/tokens'
import type { ResolvedToken } from '../auth/tokens'
import { getAllSessions } from '../pty/session-core'

async function resolveCaller(c: import('hono').Context): Promise<ResolvedToken | null> {
  const auth = c.req.header('Authorization') || ''
  const token = auth.startsWith('Bearer ') ? auth.slice(7) : ''
  return token ? resolveToken(token) : null
}

export function findOwnedLiveSession<T extends { tokenId: string; ws?: unknown }>(
  sessions: readonly T[],
  tokenId: string,
): T | undefined {
  return sessions.find(session => Boolean(session.ws) && session.tokenId === tokenId)
}

export function createHooksRoutes(): Hono {
  const app = new Hono()

  // Auth gate for the management/UI endpoints below. The CLI webhook
  // (`/:sessionId/:event/:hookId`) authenticates with the per-session
  // mcpToken and is intentionally NOT covered here. Everything else —
  // /test (spawns bash), /emit (fires host command hooks), /library, /log,
  // /merged, /active — requires a valid API token. These used to be wide
  // open: an unauthenticated caller could run arbitrary server-side shell.
  const requireApiToken = async (
    c: import('hono').Context,
    next: import('hono').Next,
  ): Promise<Response | void> => {
    if (!(await resolveCaller(c))) {
      return c.json({ error: 'Unauthorized' }, 401)
    }
    return next()
  }
  app.use('/api/hooks/library', requireApiToken)
  app.use('/api/hooks/log', requireApiToken)
  app.use('/api/hooks/log/clear', requireApiToken)
  app.use('/api/hooks/emit/*', requireApiToken)
  app.use('/api/hooks/test', requireApiToken)
  app.use('/api/hooks/merged', requireApiToken)
  app.use('/api/hooks/active/*', requireApiToken)

  app.post('/api/hooks/:sessionId/:event/:hookId', async (c) => {
    const { sessionId, hookId } = c.req.param()

    // Auth: same bearer token as MCP for this session.
    const auth = c.req.header('Authorization') || ''
    const token = auth.startsWith('Bearer ') ? auth.slice(7) : ''
    const session = getSession(sessionId)
    if (!session) {
      warnLog('Hook webhook for unknown session', { sessionId, hookId })
      return c.json({ error: 'Unknown session' }, 404)
    }
    if (!token || token !== session.mcpToken) {
      return c.json({ error: 'Unauthorized' }, 401)
    }

    let payload: HookInputPayload
    try {
      payload = (await c.req.json()) as HookInputPayload
    } catch {
      return c.json({ error: 'Invalid JSON' }, 400)
    }

    // Deterministic session status: the FIRING of these CLI lifecycle events is
    // the authoritative signal (vs scraping OSC spinner glyphs). Emit before any
    // dispatch — idempotent even if the user also has a hook for the same event.
    const event = c.req.param('event')
    if (isBridgeStatusEvent(event)) {
      applyDeterministicStatus(session, event, payload)
      // Our own no-op status hook has nothing to run — skip the dispatch (and
      // its shell spawn). A user's real hook for the same event still dispatches.
      const reg = listSession(sessionId).find(h => h.id === hookId)
      if (reg?.source.kind === 'bridge') {
        return c.json({ continue: true, suppressOutput: true }, 200)
      }
    }

    // CLI runs in a synthetic container directory; local hooks must receive
    // the real client project cwd both as process cwd and in their stdin JSON.
    if (session.cwd) payload.cwd = session.cwd
    const result = await dispatchHook(sessionId, hookId, payload, session.ws ?? null, session.tokenId)
    c.header('X-Bridge-Hook-Exit-Code', String(result.exitCode))

    // Map our HookExecutionResult to a CLI-compatible response. CLI accepts:
    //   - non-2xx HTTP                                 → treated as hook error
    //   - 2xx with JSON body matching HookOutput shape → parsed & applied
    //   - 2xx with plain text                          → printed as systemMessage
    if (result.jsonOutput) {
      // Hook returned a structured JSON — pass through unchanged.
      return c.json(result.jsonOutput, 200)
    }
    if (result.exitCode === 2) {
      // CLI convention: exit 2 = block. Mirror as decision: block.
      return c.json({
        continue: false,
        stopReason: result.stderr || result.stdout || 'Hook blocked',
        decision: 'block',
      }, 200)
    }
    if (result.outcome === 'success') {
      // Plain text stdout — surface as systemMessage if non-empty.
      return c.json({
        continue: true,
        suppressOutput: !result.stdout,
        systemMessage: result.stdout || undefined,
      }, 200)
    }
    // Error / timeout / cancelled
    return c.json({
      continue: true,
      systemMessage: `Hook error (${result.outcome}): ${result.stderr || result.stdout}`,
    }, 200)
  })

  // ── Read-only management endpoints (UI consumes these) ───────
  // Library templates — static catalog the UI Library tab renders.
  app.get('/api/hooks/library', (c) => {
    return c.json({ templates: HOOK_LIBRARY })
  })

  // Recent executions — audit log entries newest-first.
  app.get('/api/hooks/log', async (c) => {
    const limit = Math.max(1, Math.min(1000, parseInt(c.req.query('limit') ?? '200', 10) || 200))
    const caller = await resolveCaller(c)
    if (!caller) return c.json({ error: 'Unauthorized' }, 401)
    return c.json({ entries: await readRecentExecutions(limit, caller.tokenId) })
  })

  // Fire a bridge-emit event (any of the 7 custom events). Used both by
  // bridge-internal code and by the VSIX host when it wants to notify
  // renderer-side handlers that something happened in the local environment
  // (auto-update found, plugin installed, etc).
  app.post('/api/hooks/emit/:event', async (c) => {
    const event = c.req.param('event')
    if (!(BRIDGE_HOOK_EVENTS as readonly string[]).includes(event)) {
      return c.json({ ok: false, error: `Unknown bridge event: ${event}` }, 400)
    }
    let payload: Record<string, unknown> = {}
    try { payload = await c.req.json() } catch { /* allow empty body */ }
    const caller = await resolveCaller(c)
    if (!caller) return c.json({ error: 'Unauthorized' }, 401)
    const ownedSessionIds = getAllSessions()
      .filter(session => session.tokenId === caller.tokenId)
      .map(session => session.id)
    const requestedSessionId = typeof payload.sessionId === 'string' ? payload.sessionId : undefined
    if (requestedSessionId && !ownedSessionIds.includes(requestedSessionId)) {
      return c.json({ ok: false, error: 'Session not found' }, 404)
    }
    const targets = requestedSessionId ? [requestedSessionId] : ownedSessionIds
    await Promise.all(targets.map(sessionId => emitBridgeEvent(event as BridgeHookEvent, { ...payload, sessionId })))
    return c.json({ ok: true })
  })

  app.post('/api/hooks/log/clear', async (c) => {
    const caller = await resolveCaller(c)
    if (!caller) return c.json({ error: 'Unauthorized' }, 401)
    await clearLog(caller.tokenId)
    return c.json({ ok: true })
  })

  // Test-run a draft hook with optional mock payload. Bypasses the
  // session registry — spawns a one-shot dispatch matching the same
  // behaviour the real hook would have when fired from CLI.
  app.post('/api/hooks/test', async (c) => {
    let draft: { event: string; matcher?: string; handler: import('./types').HookHandler; mockPayload?: Record<string, unknown> }
    try { draft = await c.req.json() } catch { return c.json({ ok: false, error: 'Invalid JSON' }, 400) }
    if (!draft?.handler || !draft.event) return c.json({ ok: false, error: 'event and handler required' }, 400)

    const { resolveHost } = await import('./host-resolver')
    const { dispatchHook: _ } = await import('./dispatcher')
    void _
    const { registerSessionHooks, clearSession } = await import('./registry')
    const { dispatchHook } = await import('./dispatcher')

    // Allocate a throwaway session id for the registry
    const fakeSessionId = `__test__${Date.now()}`
    try {
      const fakeHooks = { [draft.event]: [{ matcher: draft.matcher, hooks: [draft.handler] }] } as import('./types').HookSettings
      const reg = registerSessionHooks(fakeSessionId, fakeHooks, { kind: 'user' })
      const hookId = Array.from(reg.values())[0]?.id
      if (!hookId) return c.json({ ok: false, error: 'Failed to register draft' }, 500)

      const payload: import('./types').HookInputPayload = {
        hook_event_name: draft.event,
        session_id: fakeSessionId,
        ...(draft.mockPayload ?? {}),
      }
      // Resolve host explicitly so we route via the right path
      const effective = resolveHost(draft.event as import('./types').HookEvent, draft.handler, draft.matcher)
      // Local hooks need an active session WS — for tests we fall back to
      // server-side dispatch when no session is around.
      let ws: import('ws').WebSocket | null = null
      const caller = await resolveCaller(c)
      const sessionsArr = Array.from((await import('../pty/session-core')).sessions.values())
      // Never borrow an arbitrary live WebSocket: that would execute the
      // caller's draft command on another tenant's client host. The auth
      // middleware guarantees `caller`, and ownership is still checked here
      // at the exact session-selection sink.
      const live = caller ? findOwnedLiveSession(sessionsArr, caller.tokenId) : undefined
      if (live && effective === 'local') ws = live.ws ?? null

      const result = await dispatchHook(fakeSessionId, hookId, payload, ws, caller?.tokenId)
      return c.json({ ok: true, result, effectiveHost: effective })
    } finally {
      clearSession(fakeSessionId)
    }
  })

  // Merged view of all 3 sources (user-global + project + plugins) with the
  // matcher → source mapping. UI Active tab renders this so users see
  // hooks coming from plugins / projects, not just user-global.
  app.get('/api/hooks/merged', async (c) => {
    const cwd = c.req.query('cwd') || undefined
    const { mergeAllHooks } = await import('./source-merger')
    const merged = mergeAllHooks(cwd)
    // Convert sourceByMatcher Map to a parallel array indexed the same way
    // the hooks object iterates so the renderer can reconstruct it.
    const flat: Array<{ event: string; matcherIdx: number; matcher?: string; handler: import('./types').HookHandler; source: import('./types').HookSource }> = []
    for (const [event, matchers] of Object.entries(merged.hooks)) {
      if (!Array.isArray(matchers)) continue
      matchers.forEach((m, mi) => {
        const source = merged.sourceByMatcher.get(m) ?? { kind: 'user' as const }
        m.hooks.forEach(h => {
          flat.push({ event, matcherIdx: mi, matcher: m.matcher, handler: h, source })
        })
      })
    }
    return c.json({ hooks: flat })
  })

  // Active hooks for a specific session (what's registered now via the
  // session's settings.json). UI uses this to render the Active tab.
  app.get('/api/hooks/active/:sessionId', async (c) => {
    const sessionId = c.req.param('sessionId')
    const caller = await resolveCaller(c)
    if (!caller) return c.json({ error: 'Unauthorized' }, 401)
    const session = getSession(sessionId)
    if (!session || session.tokenId !== caller.tokenId) return c.json({ error: 'Session not found' }, 404)
    const hooks = listSession(sessionId).map(h => ({
      id: h.id,
      event: h.event,
      matcher: h.matcher,
      handler: h.handler,
      source: h.source,
      effectiveHost: h.effectiveHost,
    }))
    return c.json({ hooks })
  })

  return app
}
