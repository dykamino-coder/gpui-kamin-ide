// Embedding example: an application's own HTTP server in front of the
// UI-less workspace host. It serves its own page and the browser client, signs
// users in, and reverse-proxies the workspace routes on the same origin.
//
//   PUBLIC_ORIGIN       exact origin browsers use, e.g. http://localhost:8080 (required)
//   BRIDGE_SERVER_URL   Bridge server, e.g. ws://bridge-server:3456 (required)
//   DEMO_PASSWORD       password of the demo sign-in form (required)
//   PORT                public listen port inside the container (8080)
//   WORKSPACE_ROOT      project directory (/workspace)
//   WORKSPACE_STORAGE   private host state (~/.local/state/bridge)
//   BRIDGE_TOKEN / BRIDGE_TOKEN_FILE   optional one-time bootstrap credential
//   PREVIEW_ENTRY       optional container-local remoteEntry.js to mount at start
import { createServer, request } from 'node:http'
import { connect } from 'node:net'
import { timingSafeEqual } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { createRequire } from 'node:module'
import { startHeadlessWorkspace } from '@kaminide/bridge-workspace-host'
import { createAssertionAuth } from './auth.mjs'

const required = name => {
  const value = process.env[name]
  if (!value) throw new Error(`${name} is required`)
  return value
}
const publicOrigin = new URL(required('PUBLIC_ORIGIN')).origin
const serverUrl = required('BRIDGE_SERVER_URL')
const password = Buffer.from(required('DEMO_PASSWORD'))
const port = Number(process.env.PORT || 8080)
const internalPort = 3460
const here = import.meta.dirname
const clientDirectory = path.dirname(createRequire(import.meta.url).resolve('@kaminide/bridge-browser-client'))

const auth = createAssertionAuth()
// The host listens on loopback only; every browser request goes through this server.
const workspace = await startHeadlessWorkspace({
  root: process.env.WORKSPACE_ROOT || '/workspace',
  storage: process.env.WORKSPACE_STORAGE || path.join(os.homedir(), '.local/state/bridge'),
  serverUrl, origin: publicOrigin, bind: '127.0.0.1', port: internalPort, auth: auth.policy,
})
if (process.env.PREVIEW_ENTRY) {
  workspace.preview.mount({ name: 'demo', entry: process.env.PREVIEW_ENTRY, exposedModule: './App' })
}

const proxied = url => url === '/bridge' || url === '/health' || url.startsWith('/auth/') || url.startsWith('/preview/')
const files = {
  '/': ['index.html', here + '/public', 'text/html; charset=utf-8'],
  '/app.js': ['app.js', here + '/public', 'text/javascript; charset=utf-8'],
  '/client/index.js': ['index.js', clientDirectory, 'text/javascript; charset=utf-8'],
}

async function siteLogin(request, response) {
  let body = ''
  for await (const chunk of request) { body += chunk; if (body.length > 4096) throw new Error('Request too large') }
  const { user, password: candidate } = JSON.parse(body)
  const given = Buffer.from(String(candidate ?? ''))
  const valid = typeof user === 'string' && /^[\w.-]{1,64}$/.test(user) && given.length === password.length && timingSafeEqual(given, password)
  response.writeHead(valid ? 200 : 401, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' })
  // The browser posts this one-time assertion to the workspace's /auth/login.
  response.end(JSON.stringify(valid ? { assertion: auth.mint(user) } : { error: 'Sign-in failed' }))
}

const server = createServer((incoming, response) => {
  const url = incoming.url ?? '/'
  if (proxied(url)) {
    const upstream = request({ host: '127.0.0.1', port: internalPort, method: incoming.method, path: url, headers: incoming.headers }, reply => {
      response.writeHead(reply.statusCode ?? 502, reply.headers)
      reply.pipe(response)
    })
    upstream.on('error', () => { if (!response.headersSent) response.writeHead(502); response.end() })
    incoming.pipe(upstream)
    return
  }
  if (url === '/site/login' && incoming.method === 'POST') {
    siteLogin(incoming, response).catch(() => { if (!response.headersSent) response.writeHead(400); response.end() })
    return
  }
  const file = files[url.split('?')[0]]
  if (!file || incoming.method !== 'GET') { response.writeHead(404); response.end(); return }
  readFile(path.join(file[1], file[0])).then(bytes => {
    response.writeHead(200, { 'Content-Type': file[2], 'Cache-Control': 'no-store' })
    response.end(bytes)
  }, () => { response.writeHead(404); response.end() })
})

// WebSocket upgrades (/bridge and preview HMR) are forwarded byte for byte.
server.on('upgrade', (incoming, socket, head) => {
  if (!proxied(incoming.url ?? '')) { socket.destroy(); return }
  const upstream = connect(internalPort, '127.0.0.1', () => {
    const lines = [`${incoming.method} ${incoming.url} HTTP/1.1`]
    for (let i = 0; i < incoming.rawHeaders.length; i += 2) lines.push(`${incoming.rawHeaders[i]}: ${incoming.rawHeaders[i + 1]}`)
    upstream.write(lines.join('\r\n') + '\r\n\r\n')
    if (head.length) upstream.write(head)
    socket.pipe(upstream).pipe(socket)
  })
  upstream.on('error', () => socket.destroy())
  socket.on('error', () => upstream.destroy())
})

server.listen(port, '0.0.0.0', () => process.stdout.write(`Embedding example listening on ${publicOrigin}\n`))
const close = () => { server.close(); void workspace.close() }
process.once('SIGINT', close)
process.once('SIGTERM', close)
