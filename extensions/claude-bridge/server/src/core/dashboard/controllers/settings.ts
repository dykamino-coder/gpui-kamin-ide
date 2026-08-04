// ============================================================================
// Dashboard proxy settings endpoints
// ============================================================================

import type { Hono } from 'hono'
import { getEditableSettings, getProxySettings, setSetting, saveCaCert, resetSettings, type SettingKey } from '../../config/settings'
import { denyApiToken } from '../authz'

export function registerSettingsRoutes(api: Hono): void {
  // GET /api/dashboard/proxy-settings -- all editable settings
  api.get('/api/dashboard/proxy-settings', (c) => {
    return c.json(getEditableSettings())
  })

  // PUT /api/dashboard/proxy-settings -- update editable settings.
  // Admin-only: this rewrites the global upstream proxy (httpProxy/httpsProxy)
  // that ALL users' /v1/messages traffic flows through — a per-user API token
  // must never be able to redirect everyone's Anthropic credentials + prompts.
  api.put('/api/dashboard/proxy-settings', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    let body: Record<string, string | number | boolean | null>
    try {
      body = await c.req.json() as Record<string, string | number | boolean | null>
    } catch {
      return c.json({ error: 'Invalid JSON body' }, 400)
    }
    const validKeys: SettingKey[] = [
      'httpProxy', 'httpsProxy', 'noProxy', 'caCert',
      'logLevel',
    ]
    for (const key of validKeys) {
      if (key in body) {
        const val = body[key]
        setSetting(key, val === null || val === '' ? null : String(val))
      }
    }
    return c.json(getEditableSettings())
  })

  // DELETE /api/dashboard/proxy-settings -- reset all settings to defaults
  api.delete('/api/dashboard/proxy-settings', (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    resetSettings()
    return c.json(getEditableSettings())
  })

  // POST /api/dashboard/proxy-settings/cert -- upload CA cert file (admin-only:
  // a caller-supplied CA cert changes TLS trust for the whole proxy).
  api.post('/api/dashboard/proxy-settings/cert', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const formData = await c.req.formData()
    const file = formData.get('cert') as File | null
    if (!file) return c.json({ error: 'No cert file provided' }, 400)
    const buffer = Buffer.from(await file.arrayBuffer())
    const filePath = saveCaCert(file.name, buffer)
    return c.json({ path: filePath, ...getProxySettings() })
  })
}
