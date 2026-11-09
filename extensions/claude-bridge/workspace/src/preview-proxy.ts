// HTTP and raw WebSocket forwarding to a container-local dev server only.
import { request as httpRequest, type IncomingMessage, type ServerResponse } from 'node:http'
import type { Duplex } from 'node:stream'

const HOP = new Set(['connection', 'keep-alive', 'proxy-authenticate', 'proxy-authorization', 'te', 'trailer', 'transfer-encoding', 'upgrade'])

function headers(request: IncomingMessage, port: number) {
  const blocked = new Set([...HOP, ...(request.headers.connection ?? '').split(',').map(x => x.trim().toLowerCase()),
    'cookie', 'authorization', 'host', 'origin', 'forwarded', 'x-forwarded-host', 'x-forwarded-proto', 'x-forwarded-for'])
  const result = Object.fromEntries(Object.entries(request.headers).filter(([key]) => !blocked.has(key)))
  result.host = `127.0.0.1:${port}`
  // Dev servers that check Origin see their own local origin. Browser origin
  // authentication has already happened at the workspace boundary.
  if (request.headers.origin) result.origin = `http://127.0.0.1:${port}`
  return result
}

export function proxyHttp(request: IncomingMessage, response: ServerResponse, target: { port: number; path: string }, origin: string, base: string): void {
  const { port, path } = target
  const upstream = httpRequest({ host: '127.0.0.1', port, path, method: request.method,
    headers: headers(request, port) }, incoming => {
    const outgoing = { ...incoming.headers }
    for (const key of [...HOP, 'set-cookie', 'access-control-allow-origin', 'access-control-allow-credentials']) delete outgoing[key]
    outgoing['cache-control'] = 'no-store'
    outgoing['referrer-policy'] = 'no-referrer'
    outgoing['x-content-type-options'] = 'nosniff'
    // Remotes share the UI's authority, but must not install a persistent worker.
    outgoing['service-worker-allowed'] = '/__preview/no-service-workers/'
    if (outgoing.location) {
      try {
        const location = new URL(outgoing.location, `http://127.0.0.1:${port}`)
        if (location.origin === `http://127.0.0.1:${port}`) outgoing.location = origin + base + location.pathname + location.search + location.hash
      } catch { delete outgoing.location }
    }
    response.writeHead(incoming.statusCode ?? 502, outgoing)
    incoming.pipe(response)
    incoming.on('error', () => response.destroy())
  })
  upstream.setTimeout(30_000, () => upstream.destroy(new Error('Preview upstream timed out')))
  upstream.on('error', () => {
    if (response.headersSent) response.destroy()
    else { response.writeHead(502, { 'Content-Type': 'text/plain', 'Cache-Control': 'no-store' }); response.end('Preview dev server is unavailable. Start it inside the workspace container.') }
  })
  response.on('close', () => upstream.destroy())
  request.pipe(upstream)
}

export function proxyUpgrade(request: IncomingMessage, client: Duplex, head: Buffer, target: { port: number; path: string }): void {
  const { port, path } = target
  const outgoing = { ...headers(request, port), connection: 'Upgrade', upgrade: 'websocket' }
  const upstream = httpRequest({ host: '127.0.0.1', port, path, headers: outgoing })
  upstream.setTimeout(10_000, () => upstream.destroy())
  upstream.on('upgrade', (response, socket, upstreamHead) => {
    if (client.destroyed) { socket.destroy(); return }
    const safeHeaders = Object.entries(response.headers).filter(([key]) => !['set-cookie', 'authorization'].includes(key))
    client.write(`HTTP/1.1 ${response.statusCode} ${response.statusMessage}\r\n${safeHeaders.map(([key, value]) => `${key}: ${Array.isArray(value) ? value.join(', ') : value}`).join('\r\n')}\r\n\r\n`)
    if (upstreamHead.length) client.write(upstreamHead)
    if (head.length) socket.write(head)
    socket.setTimeout(0)
    socket.on('error', () => client.destroy())
    client.on('error', () => socket.destroy())
    client.on('close', () => socket.destroy())
    socket.on('close', () => client.destroy())
    client.pipe(socket).pipe(client)
  })
  upstream.on('response', response => { response.resume(); client.destroy() })
  upstream.on('error', () => client.destroy())
  client.on('close', () => upstream.destroy())
  upstream.end()
}
