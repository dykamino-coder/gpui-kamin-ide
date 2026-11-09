// Authenticated HTTP/WebSocket boundary. Runtime/UI composition is a separate layer.
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http'
import type { AddressInfo } from 'node:net'
import { WebSocketServer, WebSocket } from 'ws'
import { WorkspaceAuth, readSessionCookie } from './auth.js'
import { parseClientFrame, type HostAttachment, type WorkspaceBackend } from './protocol.js'
import type { WorkspacePreview } from './preview.js'
import { proxyHttp, proxyUpgrade } from './preview-proxy.js'
import type { Duplex } from 'node:stream'

const MAX_FRAME = 16 * 1024 * 1024
const MAX_BUFFER = 32 * 1024 * 1024
const MAX_INFLIGHT = 64

export interface WorkspaceServerOptions {
  auth: WorkspaceAuth
  backend: WorkspaceBackend
  /** Exact browser origin; required for non-loopback/reverse-proxy deployment. */
  origin?: string
  bind?: string
  port?: number
  revalidateMs?: number
  /** Only for an exactly pinned loopback browser origin, including port mapping. */
  autoLogin?: boolean
  /** Trusted Module Federation remote; state is owned by this host process. */
  preview?: WorkspacePreview
  /** Serve only trusted application assets; this callback never receives API routes. */
  serveApp?: (request: IncomingMessage, response: ServerResponse) => void | Promise<void>
}

function json(response: ServerResponse, status: number, value: unknown): void {
  response.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' })
  response.end(JSON.stringify(value))
}

async function readLogin(request: IncomingMessage): Promise<unknown> {
  if (request.headers['content-type']?.split(';')[0] !== 'application/json') throw new Error('Expected JSON')
  let size = 0
  const chunks: Buffer[] = []
  for await (const chunk of request) {
    size += chunk.length
    if (size > 8192) throw new Error('Request too large')
    chunks.push(chunk)
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8'))
}

export async function startWorkspaceServer(options: WorkspaceServerOptions) {
  const bind = options.bind ?? '127.0.0.1'
  if (!options.origin && bind !== '127.0.0.1' && bind !== '::1') {
    throw new Error('An explicit browser origin is required for a non-loopback bind')
  }
  let origin = options.origin ? new URL(options.origin).origin : ''
  if (options.origin && (origin !== options.origin || !/^https?:/.test(origin))) throw new Error('Invalid browser origin')
  if (options.autoLogin && origin && !['127.0.0.1', '[::1]', 'localhost'].includes(new URL(origin).hostname)) {
    throw new Error('Automatic login requires a loopback browser origin')
  }
  const sockets = new Map<WebSocket, string>()
  const alive = new Set<WebSocket>()
  const previewSockets = new Map<Duplex, string>()
  const detachPreview = options.preview?.onRemount(() => { for (const socket of previewSockets.keys()) socket.destroy() })
  const attempts = new Map<string, { count: number; reset: number }>()
  let closing = false
  let pendingUpgrades = 0
  const sameHost = (request: IncomingMessage) => request.headers.host === new URL(origin).host
  const sameOrigin = (request: IncomingMessage) => sameHost(request) && request.headers.origin === origin
  const wss = new WebSocketServer({ noServer: true, maxPayload: MAX_FRAME, perMessageDeflate: false })

  const server = createServer((request, response) => {
    response.setHeader('X-Content-Type-Options', 'nosniff')
    response.setHeader('Referrer-Policy', 'no-referrer')
    response.setHeader('Content-Security-Policy', "frame-ancestors 'none'")
    void handle(request, response).catch(() => {
      if (!response.headersSent) json(response, 400, { error: 'Invalid request' })
      else response.destroy()
    })
  })
  server.requestTimeout = 10_000
  server.headersTimeout = 10_000

  async function handle(request: IncomingMessage, response: ServerResponse): Promise<void> {
    if (closing || !sameHost(request)) { json(response, 403, { error: 'Invalid host' }); return }
    if ((request.headers.origin && !sameOrigin(request)) || request.headers['sec-fetch-site'] === 'cross-site') {
      json(response, 403, { error: 'Invalid origin' }); return
    }
    if (request.url === '/health' && request.method === 'GET') {
      // Container reachability probes must not mint browser sessions or call
      // upstream authentication. Model readiness is tested separately.
      json(response, 200, { ready: true }); return
    }
    if (request.url === '/auth/login' && request.method === 'POST') {
      if (!sameOrigin(request)) { json(response, 403, { error: 'Invalid origin' }); return }
      const address = request.socket.remoteAddress ?? ''
      const now = Date.now()
      for (const [key, value] of attempts) if (value.reset <= now) attempts.delete(key)
      const attempt = attempts.get(address) ?? { count: 0, reset: now + 60_000 }
      if (attempt.count >= 10 || (!attempts.has(address) && attempts.size >= 256)) {
        json(response, 429, { error: 'Try again later' }); return
      }
      attempt.count++
      attempts.set(address, attempt)
      const data = await readLogin(request) as { token?: unknown } | null
      const cookie = await options.auth.login(data?.token)
      if (!cookie) { json(response, 401, { error: 'Authentication failed' }); return }
      response.setHeader('Set-Cookie', options.auth.cookieHeader(cookie, origin.startsWith('https:')))
      json(response, 200, { authenticated: true })
      return
    }
    if (request.url === '/auth/logout' && request.method === 'POST') {
      if (!sameOrigin(request)) { json(response, 403, { error: 'Invalid origin' }); return }
      const cookie = readSessionCookie(request.headers.cookie)
      options.auth.logout(cookie)
      for (const [socket, id] of sockets) if (id === cookie) socket.close(4001, 'Session ended')
      for (const [socket, id] of previewSockets) if (id === cookie) socket.destroy()
      response.setHeader('Set-Cookie', options.auth.cookieHeader('', origin.startsWith('https:')).replace(/Max-Age=\d+/, 'Max-Age=0'))
      json(response, 200, { authenticated: false })
      return
    }
    if (request.url === '/auth/session' && request.method === 'GET') {
      let authenticated = await options.auth.validate(readSessionCookie(request.headers.cookie))
      if (!authenticated && options.autoLogin) {
        const cookie = await options.auth.loginLocal()
        if (cookie) {
          response.setHeader('Set-Cookie', options.auth.cookieHeader(cookie, origin.startsWith('https:')))
          authenticated = true
        }
      }
      json(response, 200, { authenticated })
      return
    }
    if (request.url?.startsWith('/preview/')) {
      if (!options.preview) { json(response, 404, { error: 'Preview is not configured' }); return }
      const session = readSessionCookie(request.headers.cookie)
      if (!await options.auth.validate(session)) { json(response, 401, { error: 'Authentication required' }); return }
      if (request.url === '/preview/state' && request.method === 'GET') {
        json(response, 200, await options.preview.snapshot()); return
      }
      const target = options.preview.target(request.url)
      if (target) {
        if (!['GET', 'HEAD'].includes(request.method ?? '') && !sameOrigin(request)) {
          json(response, 403, { error: 'Invalid preview origin' }); return
        }
        proxyHttp(request, response, target, origin, `/preview/remotes/${options.preview.copy()!.name}`); return
      }
      if (request.method !== 'POST' || !sameOrigin(request)) { json(response, 403, { error: 'Invalid preview origin' }); return }
      if (request.url === '/preview/unmount') { options.preview.unmount(); json(response, 200, null); return }
      json(response, 404, { error: 'Unknown preview endpoint' }); return
    }
    if (request.url?.startsWith('/auth/') || request.url?.startsWith('/bridge')) {
      json(response, 404, { error: 'Unknown endpoint' }); return
    }
    if (request.method !== 'GET' && request.method !== 'HEAD') { json(response, 405, { error: 'Method not allowed' }); return }
    if (options.serveApp) await options.serveApp(request, response)
    else json(response, 503, { error: 'Browser UI is not installed' })
  }

  server.on('upgrade', (request, socket, head) => {
    const refuse = () => socket.destroy()
    const target = options.preview?.target(request.url ?? '')
    if (target) {
      if (closing || !sameOrigin(request) || previewSockets.size + pendingUpgrades >= 32) { refuse(); return }
      const cookie = readSessionCookie(request.headers.cookie)
      pendingUpgrades++
      void options.auth.validate(cookie).then(valid => {
        if (!valid || closing || socket.destroyed || options.preview?.target(request.url ?? '')?.port !== target.port) { refuse(); return }
        previewSockets.set(socket, cookie!)
        socket.once('close', () => previewSockets.delete(socket))
        proxyUpgrade(request, socket, head, target)
      }, refuse).finally(() => { pendingUpgrades-- })
      return
    }
    if (closing || request.url !== '/bridge' || !sameOrigin(request) || sockets.size + pendingUpgrades >= 32) { refuse(); return }
    const cookie = readSessionCookie(request.headers.cookie)
    pendingUpgrades++
    void options.auth.validate(cookie).then(valid => {
      if (!valid || closing || socket.destroyed || sockets.size >= 32) { refuse(); return }
      wss.handleUpgrade(request, socket, head, ws => {
        sockets.set(ws, cookie!)
        wss.emit('connection', ws)
      })
    }, refuse).finally(() => { pendingUpgrades-- })
  })

  wss.on('connection', socket => {
    let inflight = 0
    let attachment: HostAttachment
    alive.add(socket)
    socket.on('pong', () => { alive.add(socket) })
    socket.on('error', () => { /* close releases the owned attachment */ })
    socket.once('close', () => { sockets.delete(socket); alive.delete(socket); attachment?.dispose() })
    try { attachment = options.backend.attach({
      async postMessage(frame) {
        if (socket.readyState !== WebSocket.OPEN) return false
        const text = JSON.stringify(frame)
        if (Buffer.byteLength(text) > MAX_FRAME || socket.bufferedAmount + Buffer.byteLength(text) > MAX_BUFFER) {
          socket.close(4008, 'Client too slow'); return false
        }
        return new Promise(resolve => socket.send(text, error => resolve(!error)))
      },
    }) } catch { socket.close(1011, 'Host attachment failed'); return }
    socket.on('message', (raw, binary) => {
      if (socket.readyState !== WebSocket.OPEN) return
      if (binary || inflight >= MAX_INFLIGHT) { socket.close(4008, 'Input limit'); return }
      let frame
      try { frame = parseClientFrame(raw.toString()) }
      catch { socket.close(4002, 'Invalid frame'); return }
      inflight++
      void Promise.resolve().then(() => attachment.receive(frame)).catch(() => {
        // Backend replies carry handler errors. Unexpected failures close the view.
        socket.close(1011, 'Host dispatch failed')
      }).finally(() => { inflight-- })
    })
  })

  let validating = false
  const timer = setInterval(() => {
    if (validating || closing) return
    for (const socket of sockets.keys()) {
      if (!alive.delete(socket)) socket.terminate()
      else if (socket.readyState === WebSocket.OPEN) socket.ping()
    }
    validating = true
    void Promise.all([...new Set([...sockets.values(), ...previewSockets.values()])].map(async cookie => {
      if (!await options.auth.validate(cookie)) {
        for (const [socket, id] of sockets) if (id === cookie) socket.close(4001, 'Session ended')
        for (const [socket, id] of previewSockets) if (id === cookie) socket.destroy()
      }
    })).finally(() => { validating = false })
  }, options.revalidateMs ?? 15_000)
  timer.unref()
  try {
    await new Promise<void>((resolve, reject) => {
      server.once('error', reject)
      server.listen(options.port ?? 3460, bind, () => { server.off('error', reject); resolve() })
    })
  } catch (error) { clearInterval(timer); detachPreview?.(); wss.close(); throw error }
  const address = server.address() as AddressInfo
  if (!origin) origin = `http://${bind === '::1' ? '[::1]' : bind}:${address.port}`
  options.preview?.blockPorts([address.port])

  return {
    origin,
    address,
    async close(): Promise<void> {
      closing = true
      detachPreview?.()
      clearInterval(timer)
      for (const socket of previewSockets.keys()) socket.destroy()
      for (const socket of sockets.keys()) socket.terminate()
      await new Promise<void>(resolve => wss.close(() => resolve()))
      await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()))
    },
  }
}
