import { Hono } from 'hono'
import { describe, expect, it, vi } from 'vitest'

// The proxy-settings routes answer with the stored state. They used to fire the
// async store writes without waiting: a PUT answered with the old values, a
// rejected CA path became an unhandled rejection behind a 200, and the cert
// upload answered `path: {}` (a Promise).

const store = vi.hoisted(() => {
  const values: Record<string, string | null> = { httpsProxy: 'http://old:1', caCert: null }
  const later = () => new Promise((r) => setTimeout(r, 5))
  return {
    values,
    setSetting: vi.fn(async (key: string, value: string | null) => {
      await later()
      if (key === 'caCert' && value === '/missing.pem') throw new Error('CA certificate not readable: /missing.pem')
      values[key] = value
    }),
    resetSettings: vi.fn(async () => {
      await later()
      for (const key of Object.keys(values)) values[key] = null
    }),
    saveCaCert: vi.fn(async (name: string) => {
      await later()
      values.caCert = `/data/certs/${name}`
      return `/data/certs/${name}`
    }),
  }
})

vi.mock('../../config/settings', () => ({
  setSetting: store.setSetting,
  resetSettings: store.resetSettings,
  saveCaCert: store.saveCaCert,
  getEditableSettings: () => ({ ...store.values }),
  getProxySettings: () => ({ ...store.values }),
}))
vi.mock('../authz', () => ({ denyApiToken: () => null }))

import { registerSettingsRoutes } from './settings'

function app() {
  const api = new Hono()
  registerSettingsRoutes(api)
  return api
}

const put = (body: unknown) =>
  app().request('/api/dashboard/proxy-settings', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })

describe('proxy settings routes', () => {
  it('answers a PUT with the values just stored', async () => {
    const res = await put({ httpsProxy: 'http://new:2' })
    expect(res.status).toBe(200)
    expect(await res.json()).toMatchObject({ httpsProxy: 'http://new:2' })
  })

  it('reports a rejected value to the caller', async () => {
    const res = await put({ caCert: '/missing.pem' })
    expect(res.status).toBe(400)
    expect(await res.json()).toMatchObject({ error: 'CA certificate not readable: /missing.pem', caCert: null })
  })

  it('answers a reset with the defaults', async () => {
    const res = await app().request('/api/dashboard/proxy-settings', { method: 'DELETE' })
    expect(await res.json()).toMatchObject({ httpsProxy: null })
  })

  it('answers a cert upload with the saved path', async () => {
    const form = new FormData()
    form.append('cert', new File(['-----BEGIN CERTIFICATE-----'], 'corp.pem'))
    const res = await app().request('/api/dashboard/proxy-settings/cert', { method: 'POST', body: form })
    expect(await res.json()).toMatchObject({ path: '/data/certs/corp.pem', caCert: '/data/certs/corp.pem' })
  })
})
