import assert from 'node:assert/strict'
import { mkdtemp, mkdir, writeFile, rm, symlink, truncate } from 'node:fs/promises'
import { request } from 'node:http'
import os from 'node:os'
import path from 'node:path'
import { test, type TestContext } from 'node:test'
import { createAppAssetHandler } from '../src/assets.js'
import { WorkspaceAuth } from '../src/auth.js'
import { startWorkspaceServer } from '../src/server.js'

async function directory(t: TestContext) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'bridgeweb-assets-'))
  assert.ok(path.resolve(root).startsWith(path.join(os.tmpdir(), 'bridgeweb-assets-')))
  t.after(() => rm(root, { recursive: true, force: true }))
  const app = path.join(root, 'app')
  await mkdir(path.join(app, 'assets'), { recursive: true })
  await writeFile(path.join(app, 'browser.html'), '<!doctype html><script src="/assets/ui-A123.js"></script>')
  await writeFile(path.join(app, 'assets/ui-A123.js'), 'globalThis.fixture = "original"')
  return { root, app }
}

async function fixture(t: TestContext) {
  const dirs = await directory(t)
  let attached = 0
  const server = await startWorkspaceServer({
    port: 0,
    auth: new WorkspaceAuth('asset-fixture-token', async () => ({ id: 'fixture' })),
    backend: { attach() { attached++; throw new Error('No backend calls expected') } },
    serveApp: await createAppAssetHandler(dirs.app),
  })
  t.after(() => server.close())
  // HTTP request preserves raw traversal/absolute-form paths that fetch would
  // normalize away before the server can exercise its rejection path.
  function get(target: string, method = 'GET', headers: Record<string, string> = {}) {
    return new Promise<{ status: number; headers: import('node:http').IncomingHttpHeaders; body: Buffer }>((resolve, reject) => {
      const req = request(server.origin, { path: target, method, headers }, response => {
        const chunks: Buffer[] = []
        response.on('data', chunk => chunks.push(chunk))
        response.once('end', () => resolve({ status: response.statusCode!, headers: response.headers, body: Buffer.concat(chunks) }))
        response.once('error', reject)
      })
      req.once('error', reject)
      req.end()
    })
  }
  return { ...dirs, server, get, attached: () => attached }
}

test('serves exact entry/assets with MIME, HEAD, query handling and no-store before login', async t => {
  const f = await fixture(t)
  const root = await f.get('/')
  assert.equal(root.status, 200)
  assert.equal(root.headers['content-type'], 'text/html; charset=utf-8')
  assert.match(root.body.toString(), /ui-A123.js/)
  assert.equal((await f.get('/browser.html')).body.toString(), root.body.toString())
  const script = await f.get('/assets/ui-A123.js?revision=test')
  assert.equal(script.status, 200)
  assert.equal(script.headers['content-type'], 'text/javascript; charset=utf-8')
  assert.equal(script.headers['cache-control'], 'no-store')
  assert.equal(script.headers['x-content-type-options'], 'nosniff')
  const head = await f.get('/assets/ui-A123.js', 'HEAD')
  assert.equal(head.status, 200)
  assert.equal(head.body.length, 0)
  assert.equal(head.headers['content-length'], String(script.body.length))
  assert.deepEqual(JSON.parse((await f.get('/auth/session')).body.toString()), { authenticated: false })
  assert.equal(f.attached(), 0)
})

test('unknown, traversal, encoded separator, source and absolute-form paths never expose project files', async t => {
  const f = await fixture(t)
  await writeFile(path.join(f.root, 'private.txt'), 'not an application asset')
  await writeFile(path.join(f.app, 'private.txt'), 'also not an application asset')
  await writeFile(path.join(f.app, 'assets/ui-A123.js.map'), '{"sourcesContent":["private source"]}')
  // Re-snapshot to prove ignored source/map files are absent at startup too.
  const serveApp = await createAppAssetHandler(f.app)
  const secondary = await startWorkspaceServer({
    port: 0, auth: new WorkspaceAuth('token', async () => null),
    backend: { attach() { throw new Error('Unused') } }, serveApp,
  })
  t.after(() => secondary.close())
  assert.equal((await fetch(secondary.origin + '/assets/ui-A123.js.map')).status, 404)
  for (const target of ['/private.txt', '/assets/missing.js', '/assets/', '/missing-route', '/assets/../browser.html',
    '/assets/%2e%2e/browser.html', '/assets/%2E%2E%2Fbrowser.html', '/assets%2f%2e%2e%2fbrowser.html',
    '//browser.html', '/assets//ui-A123.js', '/assets/./ui-A123.js', '/assets%5cui-A123.js',
    '/assets/ui-A123.js%00', '/assets/%zz', '/browser.html#fragment', 'http://attacker.test/browser.html']) {
    const response = await f.get(target)
    assert.equal(response.status, 404, target)
    assert.equal(response.body.length, 0, target)
  }
  assert.equal((await f.get('/browser.html', 'POST')).status, 405)
  assert.equal((await f.get('/', 'GET', { Host: 'attacker.test' })).status, 403)
  assert.equal((await f.get('/bridge')).status, 404)
  assert.equal((await f.get('/auth/not-an-asset')).status, 404)
})

test('running server keeps its byte snapshot when files are changed or removed', async t => {
  const f = await fixture(t)
  await writeFile(path.join(f.app, 'browser.html'), 'changed HTML')
  await writeFile(path.join(f.app, 'assets/ui-A123.js'), 'changed script')
  await writeFile(path.join(f.app, 'assets/new.js'), 'new script')
  assert.match((await f.get('/')).body.toString(), /ui-A123.js/)
  assert.match((await f.get('/assets/ui-A123.js')).body.toString(), /original/)
  assert.equal((await f.get('/assets/new.js')).status, 404)
  await rm(path.join(f.app, 'assets/ui-A123.js'))
  assert.equal((await f.get('/assets/ui-A123.js')).status, 200)
})

test('CSS, images, fonts, audio and WASM preserve exact bytes and content types', async t => {
  const { app } = await directory(t)
  const expected: Record<string, string> = { css: 'text/css; charset=utf-8', svg: 'image/svg+xml',
    png: 'image/png', woff2: 'font/woff2', wasm: 'application/wasm', mp3: 'audio/mpeg' }
  const bytes = Buffer.from([0, 255, 1, 128])
  await mkdir(path.join(app, 'assets/nested'))
  for (const ext of Object.keys(expected)) await writeFile(path.join(app, 'assets/nested/fixture.' + ext), bytes)
  const server = await startWorkspaceServer({ port: 0, serveApp: await createAppAssetHandler(app),
    auth: new WorkspaceAuth('token', async () => null), backend: { attach() { throw new Error('Unused') } } })
  t.after(() => server.close())
  for (const [ext, mime] of Object.entries(expected)) {
    const response = await fetch(server.origin + '/assets/nested/fixture.' + ext)
    assert.equal(response.status, 200)
    assert.equal(response.headers.get('content-type'), mime)
    assert.deepEqual(Buffer.from(await response.arrayBuffer()), bytes)
  }
})

test('startup fails closed on missing entry or asset directory', async t => {
  const { app } = await directory(t)
  await rm(path.join(app, 'browser.html'))
  await assert.rejects(createAppAssetHandler(app), /ENOENT/)
  await writeFile(path.join(app, 'browser.html'), 'entry')
  await rm(path.join(app, 'assets/ui-A123.js'))
  await rm(path.join(app, 'assets'), { recursive: true })
  await assert.rejects(createAppAssetHandler(app), /ENOENT/)
})

test('startup rejects linked directories instead of publishing their contents', async t => {
  const { root, app } = await directory(t)
  const outside = path.join(root, 'project')
  await mkdir(outside)
  await writeFile(path.join(outside, 'secret.js'), 'project bytes')
  await symlink(outside, path.join(app, 'assets/link'), process.platform === 'win32' ? 'junction' : 'dir')
  await assert.rejects(createAppAssetHandler(app), /without symlinks/)
})

test('startup rejects linked entry/files on platforms with unprivileged file symlinks',
  { skip: process.platform === 'win32' }, async t => {
    const { root, app } = await directory(t)
    const outside = path.join(root, 'secret.js')
    await writeFile(outside, 'project bytes')
    await symlink(outside, path.join(app, 'assets/link.js'))
    await assert.rejects(createAppAssetHandler(app), /without symlinks/)
    await rm(path.join(app, 'assets/link.js'))
    await rm(path.join(app, 'browser.html'))
    await symlink(outside, path.join(app, 'browser.html'))
    await assert.rejects(createAppAssetHandler(app), /without symlinks/)
  })

test('startup bounds asset bytes and recursive depth', async t => {
  const { app } = await directory(t)
  const large = path.join(app, 'assets/oversized.js')
  await writeFile(large, '')
  await truncate(large, 16 * 1024 * 1024 + 1)
  await assert.rejects(createAppAssetHandler(app), /exceeds 16 MiB/)
  await rm(large)
  await mkdir(path.join(app, 'assets/a/b/c/d/e/f/g/h/i'), { recursive: true })
  await assert.rejects(createAppAssetHandler(app), /depth limit/)
})
