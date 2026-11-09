import { createServerHttp } from '../../extension/src/server-http.js'
import type { ServerFetchInit } from '../../extension/src/main/sync/request-auth.js'
import type { WorkspaceBackend } from './protocol.js'

/** Browser API callers cannot choose the HTTP destination or recover host credentials. */
export function withBrowserServer(backend: WorkspaceBackend, options: { serverUrl: string; token: string }): WorkspaceBackend {
  const url = new URL(options.serverUrl)
  url.protocol = url.protocol === 'wss:' ? 'https:' : url.protocol === 'ws:' ? 'http:' : url.protocol
  url.pathname = '/'; url.search = ''; url.hash = ''
  const http = createServerHttp(url.href, options.token, { pinned: true, maxBytes: 8 * 1024 * 1024 })
  function redact(value: unknown): unknown {
    if (typeof value === 'string') return value.split(options.token).join('[workspace credential]')
    if (Array.isArray(value)) return value.map(redact)
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, redact(item)]))
    return value
  }
  if (!options.token) throw new Error('A configured workspace token is required')
  return { attach(view) {
    const inner = backend.attach(view)
    let closed = false
    return {
      async receive(frame) {
        if (closed) throw new Error('Browser view closed')
        if (frame.kind !== 'invoke' || !['bridge:create-token', 'bridge:server-version', 'bridge:server-fetch'].includes(frame.channel)) {
          await inner.receive(frame); return
        }
        try {
          let result: unknown
          if (frame.channel === 'bridge:create-token') {
            if (typeof frame.args[1] !== 'string') throw new Error('Expected a token name')
            result = await http.createToken(frame.args[1])
          } else if (frame.channel === 'bridge:server-version') result = await http.serverVersion()
          else {
            const route = frame.args[1]
            if (typeof route !== 'string') throw new Error('Expected a Bridge API path')
            let init = frame.args[2] as ServerFetchInit | undefined
            if (init && (typeof init !== 'object' || Array.isArray(init))) throw new Error('Invalid HTTP request')
            if (init?.headers) init = { ...init, headers: Object.fromEntries(Object.entries(init.headers).map(([key, value]) => [key,
              key.toLowerCase() === 'authorization' && value === 'Bearer [workspace credential]' ? `Bearer ${options.token}` : value])) }
            if (route === '/api/dashboard/tokens/resolve' && init?.body) {
              const body = JSON.parse(init.body)
              if (body.token === '[workspace credential]') init = { ...init, body: JSON.stringify({ ...body, token: options.token }) }
            }
            result = await http.serverFetch(route, init)
          }
          if (!closed) await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result: redact(result) })
        } catch {
          if (!closed) await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: false, error: 'Bridge server request failed' })
        }
      },
      dispose() { if (!closed) { closed = true; inner.dispose() } },
    }
  } }
}
