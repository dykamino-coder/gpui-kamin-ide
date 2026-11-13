// ============================================================================
// Dashboard API Token endpoints
// ============================================================================

import type { Context, Hono } from 'hono'
import { createToken, deleteToken, findTokenByValue, listTokens, maskToken, updateToken } from '../../auth/tokens'
import { validateDashboardSession } from '../../auth/dashboard-auth'
import {
  getStreamingSettings,
  setStreamingSettings,
  invalidateStreamingSettingsCache,
  DEFAULT_STREAMING_SETTINGS,
  type StreamingSettings,
} from '../../proxy/streaming-settings'

export function registerTokenRoutes(api: Hono): void {
  // Token routes bypass auth middleware, so resolve the admin session directly
  // from the header to preserve the existing list/copy contract.
  async function isAdminSession(c: Context): Promise<boolean> {
    const auth = c.req.header('authorization')
    if (!auth?.startsWith('Bearer ')) return false
    return !!(await validateDashboardSession(auth.slice(7)))
  }

  api.get('/api/dashboard/tokens', async (c) => {
    const tokens = await listTokens()
    if (await isAdminSession(c)) return c.json(tokens)
    return c.json(tokens.map((t) => ({ ...t, token: maskToken(t.token) })))
  })

  // POST /api/dashboard/tokens/resolve — resolve a caller-presented token value
  // to its own { id, name, createdAt }. Proof-of-possession (you must send the
  // token), so this is not a leak, and it replaces the old "list all + match by
  // value" pattern that forced raw tokens into the list response.
  api.post('/api/dashboard/tokens/resolve', async (c) => {
    let body: { token?: string }
    try {
      body = (await c.req.json()) as { token?: string }
    } catch {
      return c.json({ error: 'Invalid JSON body' }, 400)
    }
    const info = await findTokenByValue(body.token?.trim() || '')
    if (!info) return c.json({ error: 'Token not found' }, 404)
    return c.json(info)
  })

  // POST /api/dashboard/tokens — create a new user token (public: the first-run
  // gate has no token yet). Idempotent by name: an already-existing name returns
  // its existing token so a reinstall on the same machine auto-recovers the
  // token by POSTing its machineId.
  // Credential issuance/recovery remains the existing client setup contract.
  api.post('/api/dashboard/tokens', async (c) => {
    const body = (await c.req.json()) as { name?: string }
    const name = body.name?.trim()

    if (!name) {
      return c.json({ error: 'name is required' }, 400)
    }

    const existing = (await listTokens()).find((t) => t.name === name)
    if (existing) {
      return c.json(existing, 200)
    }

    const token = await createToken(name)
    return c.json(token, 201)
  })

  api.patch('/api/dashboard/tokens/:id', async (c) => {
    const body = (await c.req.json()) as { name?: string }
    const name = body.name?.trim()
    if (!name) return c.json({ error: 'name is required' }, 400)
    const token = await updateToken(c.req.param('id'), { name })
    if (!token) return c.json({ error: 'Token not found' }, 404)
    if (await isAdminSession(c)) return c.json(token)
    return c.json({ ...token, token: maskToken(token.token) })
  })

  // DELETE /api/dashboard/tokens/:id — delete a token
  api.delete('/api/dashboard/tokens/:id', async (c) => {
    const id = c.req.param('id')
    const deleted = await deleteToken(id)
    if (!deleted) return c.json({ error: 'Token not found' }, 404)
    return c.json({ ok: true })
  })

  // GET /api/dashboard/tokens/:tokenId/streaming-settings — per-user
  // streaming toggles. Resolves to defaults when no row exists.
  api.get('/api/dashboard/tokens/:tokenId/streaming-settings', async (c) => {
    const tokenId = c.req.param('tokenId')
    try {
      const settings = await getStreamingSettings(tokenId)
      return c.json(settings)
    } catch (err) {
      return c.json({ ...DEFAULT_STREAMING_SETTINGS, error: String(err) }, 200)
    }
  })

  // PUT /api/dashboard/tokens/:tokenId/streaming-settings
  api.put('/api/dashboard/tokens/:tokenId/streaming-settings', async (c) => {
    const tokenId = c.req.param('tokenId')
    const body = (await c.req.json()) as Partial<StreamingSettings>
    const current = await getStreamingSettings(tokenId)
    const merged: StreamingSettings = {
      enabled: typeof body.enabled === 'boolean' ? body.enabled : current.enabled,
      showSideQuests: typeof body.showSideQuests === 'boolean' ? body.showSideQuests : current.showSideQuests,
      showThinkingStream:
        typeof body.showThinkingStream === 'boolean' ? body.showThinkingStream : current.showThinkingStream,
      captureRequestBodies:
        typeof body.captureRequestBodies === 'boolean' ? body.captureRequestBodies : current.captureRequestBodies,
    }
    await setStreamingSettings(tokenId, merged)
    invalidateStreamingSettingsCache(tokenId)
    return c.json(merged)
  })
}
