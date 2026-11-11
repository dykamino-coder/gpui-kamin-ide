import assert from 'node:assert/strict'
import { fork, spawn, spawnSync, type ChildProcess } from 'node:child_process'
import { EventEmitter, once } from 'node:events'
import fs from 'node:fs'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import os from 'node:os'
import path from 'node:path'
import { test, type TestContext } from 'node:test'
import { fileURLToPath } from 'node:url'
import { WebSocket, WebSocketServer } from 'ws'

const here = fileURLToPath(new URL('.', import.meta.url))
const dist = path.join(here, '../dist')

async function freePort(): Promise<number> {
  const probe = createServer()
  await new Promise<void>(resolve => probe.listen(0, '127.0.0.1', resolve))
  const { port } = probe.address() as AddressInfo
  await new Promise(resolve => probe.close(resolve))
  return port
}

/** Temporary root/HOME and a synthetic Bridge server (token resolve + session socket). */
async function sandbox(t: TestContext) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-headless-'))
  const home = path.join(root, 'home'), tmp = path.join(root, 'tmp')
  fs.mkdirSync(home); fs.mkdirSync(tmp)
  const cleanup: Array<() => void | Promise<void>> = []
  t.after(async () => {
    for (const close of cleanup.reverse()) await close()
    assert.equal(path.dirname(root), fs.realpathSync(os.tmpdir()))
    assert.ok(path.basename(root).startsWith('bridgeweb-headless-'))
    fs.rmSync(root, { recursive: true, force: true })
  })
  const http: Server = createServer((request, response) => {
    request.resume()
    response.setHeader('Content-Type', 'application/json')
    response.end(request.url === '/api/dashboard/tokens/resolve' ? '{"id":"synthetic-identity"}' : '{"ok":true}')
  })
  const upstream = new WebSocketServer({ server: http })
  await new Promise<void>(resolve => http.listen(0, '127.0.0.1', resolve))
  cleanup.push(async () => {
    for (const socket of upstream.clients) socket.terminate()
    await new Promise<void>(resolve => upstream.close(() => resolve()))
    await new Promise<void>(resolve => http.close(() => resolve()))
  })
  const env = { PATH: process.env.PATH, SystemRoot: process.env.SystemRoot, ComSpec: process.env.ComSpec,
    ProgramFiles: process.env.ProgramFiles, HOME: home, USERPROFILE: home, TEMP: tmp, TMP: tmp, TMPDIR: tmp,
    KAMIN_CACHE_DIR: path.join(root, 'cache'), KAMIN_DISABLE_EXTERNAL_MCP: '1' }
  const stop = (child: ChildProcess) => cleanup.push(async () => {
    if (child.exitCode === null && child.signalCode === null) {
      if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true })
      else child.kill()
      await once(child, 'exit', { signal: AbortSignal.timeout(5000) })
    }
  })
  return { root, tmp, env, stop, cleanup, upstreamUrl: `ws://127.0.0.1:${(http.address() as AddressInfo).port}` }
}

function frames(socket: WebSocket) {
  const list: any[] = []
  const events = new EventEmitter()
  socket.on('message', raw => { list.push(JSON.parse(raw.toString())); events.emit('frame') })
  return async (match: (frame: any) => boolean) => {
    const deadline = AbortSignal.timeout(10_000)
    for (;;) {
      const found = list.find(match)
      if (found) { list.splice(list.indexOf(found), 1); return found }
      await once(events, 'frame', { signal: deadline })
    }
  }
}

test('headless host starts with an external auth policy, guards /preview/state and pushes preview state over /bridge', async t => {
  const box = await sandbox(t)
  const port = await freePort()
  const child = fork(path.join(here, 'fixtures/headless-child.cjs'), [box.root, box.upstreamUrl, String(port)],
    { cwd: box.tmp, execArgv: [], stdio: ['ignore', 'pipe', 'pipe', 'ipc'], env: box.env })
  child.stdout!.resume(); child.stderr!.resume()
  box.stop(child)
  const exited = once(child, 'exit')
  const [ready] = await once(child, 'message', { signal: AbortSignal.timeout(15_000) }) as [{ origin: string; version: string; bootstrapConsumed: boolean; error?: string }]
  assert.equal(ready.error, undefined)
  const origin = `http://127.0.0.1:${port}`
  assert.equal(ready.origin, origin)
  assert.equal(ready.bootstrapConsumed, true)
  assert.equal(ready.version, JSON.parse(fs.readFileSync(path.join(here, '../../../../builtin-extensions/claude-bridge/package.json'), 'utf8')).version)

  // No UI is installed and the embedder's policy, not the Bridge token, signs in.
  assert.equal((await fetch(`${origin}/`)).status, 503)
  const login = (token: string) => fetch(`${origin}/auth/login`, { method: 'POST', headers: { Origin: origin, 'Content-Type': 'application/json' }, body: JSON.stringify({ token }) })
  assert.equal((await login('synthetic-bootstrap-token')).status, 401)
  const accepted = await login('synthetic-studio-assertion')
  assert.equal(accepted.status, 200)
  const cookie = accepted.headers.get('set-cookie')!.split(';')[0]
  assert.equal((await fetch(`${origin}/preview/state`)).status, 401)
  const state = await fetch(`${origin}/preview/state`, { headers: { Cookie: cookie } })
  assert.equal(state.status, 200)
  assert.deepEqual(await state.json(), { descriptor: null, build: null, available: false })

  const refused = new WebSocket(`ws://127.0.0.1:${port}/bridge`, { headers: { Origin: origin } })
  await new Promise<void>(resolve => refused.once('error', () => resolve()))
  const socket = new WebSocket(`ws://127.0.0.1:${port}/bridge`, { headers: { Origin: origin, Cookie: cookie } })
  const next = frames(socket)
  await once(socket, 'open')
  const preview = (match: (state: any) => boolean) => next(frame => frame.kind === 'event' && frame.channel === 'workspace:preview-state' && match(frame.args[0]))
  assert.deepEqual((await preview(() => true)).args[0], { descriptor: null, build: null, available: false })
  socket.send(JSON.stringify({ kind: 'invoke', id: 1, generation: 'headless-test', channel: 'get-version', args: [] }))
  assert.equal((await next(frame => frame.kind === 'invoke-reply' && frame.id === 1)).result, ready.version)

  child.send('mount')
  const mounted = (await preview(value => value.available)).args[0]
  assert.equal(mounted.descriptor.name, 'demo')
  assert.equal(mounted.descriptor.entry, '/preview/remotes/demo/remoteEntry.js')
  child.send('rebuild')
  const rebuilt = (await preview(value => value.available && value.build !== mounted.build)).args[0]
  assert.equal(rebuilt.descriptor.revision, mounted.descriptor.revision)

  socket.terminate()
  child.send('shutdown')
  await once(child, 'message', { signal: AbortSignal.timeout(10_000) })
  assert.deepEqual(await Promise.race([exited, new Promise(resolve => setTimeout(() => resolve('timeout'), 10_000))]), [0, null])
})

test('cli runs the same dist/headless.cjs composition with Bridge-token sign-in and the bundled UI', async t => {
  const cli = fs.readFileSync(path.join(dist, 'cli.cjs'), 'utf8')
  assert.match(cli, /require\("\.\/headless\.cjs"\)/)
  for (const internal of ['createSharedBackend', 'BridgeHost', 'startWorkspaceServer']) assert.equal(cli.includes(internal), false, internal)

  const box = await sandbox(t)
  const ui = path.join(box.root, 'ui')
  fs.mkdirSync(path.join(ui, 'assets'), { recursive: true })
  fs.writeFileSync(path.join(ui, 'browser.html'), '<!doctype html><title>fixture UI</title>')
  const port = await freePort()
  const project = path.join(box.root, 'project')
  fs.mkdirSync(project)
  const child = spawn(process.execPath, [path.join(dist, 'cli.cjs')], { cwd: box.tmp, stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...box.env, WORKSPACE_ROOT: project, WORKSPACE_STORAGE: path.join(box.root, 'storage'), WORKSPACE_PORT: String(port),
      BRIDGE_SERVER_URL: box.upstreamUrl, BRIDGE_TOKEN: 'synthetic-workspace-token', WORKSPACE_UI_DIR: ui } })
  box.stop(child)
  child.stderr!.resume()
  let output = ''
  child.stdout!.on('data', chunk => { output += chunk })
  const deadline = Date.now() + 15_000
  while (!output.includes('Bridge workspace listening.')) {
    assert.ok(Date.now() < deadline && child.exitCode === null, 'cli did not start')
    await new Promise(resolve => setTimeout(resolve, 50))
  }
  const origin = `http://127.0.0.1:${port}`
  assert.match(await (await fetch(`${origin}/`)).text(), /fixture UI/)
  // Loopback automatic sign-in uses the stored Bridge token, as before.
  const session = await fetch(`${origin}/auth/session`)
  assert.deepEqual(await session.json(), { authenticated: true })
  const cookie = session.headers.get('set-cookie')!.split(';')[0]
  assert.equal((await fetch(`${origin}/preview/state`, { headers: { Cookie: cookie } })).status, 200)
  const socket = new WebSocket(`ws://127.0.0.1:${port}/bridge`, { headers: { Origin: origin, Cookie: cookie } })
  const next = frames(socket)
  await once(socket, 'open')
  await next(frame => frame.channel === 'workspace:preview-state')
  socket.terminate()
  assert.equal(fs.existsSync(path.join(box.root, 'storage', 'workspace-browser-sessions.json')), true)
})

test('the packed host contains the exact cli composition and declarations generated from the source', async t => {
  const out = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-pack-'))
  t.after(() => {
    assert.equal(path.dirname(out), fs.realpathSync(os.tmpdir()))
    fs.rmSync(out, { recursive: true, force: true })
  })
  const packed = spawnSync(process.execPath, [path.join(here, '../pack-headless.mjs'), '--version', '0.0.0-test.1', '--out', out], { encoding: 'utf8' })
  assert.equal(packed.status, 0, packed.stderr)
  assert.equal(path.basename(packed.stdout.trim()), 'kaminide-bridge-workspace-host-0.0.0-test.1.tgz')
  const stage = path.join(out, 'workspace-host')
  assert.deepEqual(fs.readFileSync(path.join(stage, 'headless.cjs')), fs.readFileSync(path.join(dist, 'headless.cjs')))
  const manifest = JSON.parse(fs.readFileSync(path.join(stage, 'package.json'), 'utf8'))
  assert.equal(manifest.name, '@kaminide/bridge-workspace-host')
  assert.equal(manifest.bridge.server, JSON.parse(fs.readFileSync(path.join(here, '../../server/package.json'), 'utf8')).version)
  for (const name of ['ws', '@homebridge/node-pty-prebuilt-multiarch']) assert.ok(manifest.dependencies[name], name)
  const listing = spawnSync('tar', ['-tzf', packed.stdout.trim()], { encoding: 'utf8' })
  assert.deepEqual(listing.stdout.trim().split(/\r?\n/).sort(), ['package/README.md', 'package/headless.cjs', 'package/headless.d.ts', 'package/package.json'])

  // An embedder type-checks against the generated declarations alone.
  const ts = (await import('typescript')).default
  const consumer = path.join(out, 'consumer.ts')
  fs.writeFileSync(consumer, `
    import { startHeadlessWorkspace, createSessionCookie, sessionCookieHeader, type WorkspaceAuthPolicy } from '@kaminide/bridge-workspace-host'
    const auth: WorkspaceAuthPolicy = {
      async login() { return createSessionCookie() }, async validate() { return true }, logout() {},
      cookieHeader: (cookie, secure) => sessionCookieHeader(cookie, { secure, maxAgeSeconds: 60 }),
    }
    export const started: Promise<{ origin: string; close(): Promise<void> }> = startHeadlessWorkspace({
      root: '/workspace', storage: '/state', serverUrl: 'ws://bridge:3456', origin: 'http://studio:3460', auth })
    // @ts-expect-error auth is required
    void startHeadlessWorkspace({ root: '/workspace', storage: '/state', serverUrl: 'ws://bridge:3456', origin: 'http://studio:3460' })
  `)
  const program = ts.createProgram([consumer], { strict: true, noEmit: true, module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    target: ts.ScriptTarget.ES2022, types: ['node'], typeRoots: [path.join(here, '../node_modules/@types')],
    paths: { '@kaminide/bridge-workspace-host': [path.join(stage, 'headless.d.ts')] } })
  const diagnostics = ts.getPreEmitDiagnostics(program).map(d => ts.flattenDiagnosticMessageText(d.messageText, '\n'))
  assert.deepEqual(diagnostics, [])
})
