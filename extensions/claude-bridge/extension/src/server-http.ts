import { withSyncAuthorization, type ServerFetchInit } from './main/sync/request-auth.js'

export interface ServerHttpOptions { pinned?: boolean; maxBytes?: number }

/** Shared HTTP operations for native and browser client hosts. */
export function createServerHttp(httpBase: string, token?: string, options: ServerHttpOptions = {}) {
  const base = new URL(httpBase)
  if (!['http:', 'https:'].includes(base.protocol) || base.username || base.password) throw new Error('Invalid Bridge HTTP origin')
  function target(route: string): string {
    if (options.pinned && (!route.startsWith('/') || route.startsWith('//') || route.includes('\\'))) throw new Error('Expected a relative Bridge API path')
    const url = new URL(`${base.href.replace(/\/$/, '')}${route}`)
    if (options.pinned && url.origin !== base.origin) throw new Error('Bridge HTTP origin is pinned')
    return url.href
  }
  async function request(route: string, init?: ServerFetchInit) {
    const authenticated = withSyncAuthorization(route, init, token)
    const response = await fetch(target(route), {
      method: authenticated?.method ?? 'GET', body: authenticated?.body,
      headers: authenticated?.headers ?? (authenticated?.body ? { 'Content-Type': 'application/json' } : undefined),
      signal: AbortSignal.timeout(8000), ...(options.pinned ? { redirect: 'error' as const } : {}),
    })
    let text = ''
    if (options.maxBytes && response.body) {
      const reader = response.body.getReader()
      const chunks: Uint8Array[] = []
      let size = 0
      try {
        for (;;) {
          const next = await reader.read()
          if (next.done) break
          size += next.value.byteLength
          if (size > options.maxBytes) throw new Error('Bridge response exceeds the browser limit')
          chunks.push(next.value)
        }
        text = Buffer.concat(chunks).toString('utf8')
      } finally { await reader.cancel() }
    } else text = await response.text()
    let data: unknown = null
    try { data = text ? JSON.parse(text) : null } catch { data = text }
    return { ok: response.ok, status: response.status, data }
  }
  return {
    async createToken(name: string) {
      const result = await request('/api/dashboard/tokens', { method: 'POST', body: JSON.stringify({ name }) })
      if (!result.ok) throw new Error(`HTTP ${result.status}`)
      return result.data
    },
    async serverVersion() {
      for (const route of ['/api/bridge-version', '/health']) {
        try { const result = await request(route); if (result.ok) return result.data } catch { /* try the legacy health endpoint */ }
      }
      return null
    },
    async serverFetch(route: string, init?: ServerFetchInit) {
      try { return await request(route, init) }
      catch (error) { return { ok: false, status: 0, data: null,
        error: options.pinned ? 'Bridge server request failed' : error instanceof Error ? error.message : String(error) } }
    },
  }
}
