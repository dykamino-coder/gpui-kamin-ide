// Production page and authenticated transport; fixture backend isolates the
// browser primitives. Shared-handler acceptance lives in runtime.test.ts.
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { createBrowserEnvironment, WorkspaceAuth, startWorkspaceServer, withWorkspaceShell } from '../dist/index.js'
import './linux-browser-only.mjs'

test('production browser uploads actual bytes, downloads exports and mediates clipboard', { timeout: 60_000 }, async t => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'bridgeweb-browser-effects-'))
  t.after(async () => {
    assert.equal(await fs.realpath(path.dirname(root)), await fs.realpath(os.tmpdir()))
    assert.ok(path.basename(root).startsWith('bridgeweb-browser-effects-'))
    await fs.rm(root, { recursive: true, force: true })
  })
  const assets = fileURLToPath(new URL('../../webview/dist/browser/', import.meta.url))
  const environment = createBrowserEnvironment({ root })
  const config = { serverUrl: 'ws://fixture.invalid', token: '[workspace credential]', cwd: root }
  const replies = new Map([
    ['get-version', 'fixture'], ['get-config', config], ['layout:get', {}],
    ['skills:list', []], ['session:get-saved', []], ['tab:list', []], ['tabs:restore-state', []],
    ['vscode:is-available', false], ['mcp:status', []], ['mic-connected', false],
    ['get-editor-selection', null], ['bridge:server-version', null],
    ['get-machine-id', 'browser-fixture'], ['hooks:list-pending-plugin-approvals', []],
    ['base-prompt:get', ''], ['notifications:get', {}], ['terminal:get-prefs', {}],
  ])
  const unexpected = new Set()
  const backend = environment.wrap({ attach(view) { return {
    async receive(frame) {
      if (frame.kind !== 'invoke') return
      let result
      try {
        if (frame.channel === 'open-file-dialog') {
          result = await environment.environment.pickPaths({ canSelectFiles: true, canSelectMany: true })
        } else if (frame.channel === 'write-clipboard') {
          await environment.environment.clipboard.writeText(frame.args[0]); result = true
        } else if (frame.channel === 'read-clipboard') {
          result = await environment.environment.clipboard.readText()
        } else if (frame.channel === 'save-text-as') {
          const file = await environment.environment.savePath({ defaultUri: { fsPath: frame.args[0].defaultName } })
          await fs.writeFile(file, frame.args[0].text); result = { ok: true }
        } else if (replies.has(frame.channel)) result = replies.get(frame.channel)
        else { unexpected.add(frame.channel); throw new Error('Unknown fixture channel') }
        await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result })
      } catch (error) {
        await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: false, error: error.message })
      }
    }, dispose() {},
  } } })
  const server = await startWorkspaceServer({
    port: 0, backend: withWorkspaceShell(backend),
    auth: new WorkspaceAuth('effects-fixture-token', async () => ({ id: 'fixture' })),
    async serveApp(request, response) {
      const name = request.url === '/' ? 'browser.html' : request.url.slice(1)
      const file = path.resolve(assets, name)
      if (!file.startsWith(assets)) { response.writeHead(404); response.end(); return }
      try {
        const content = await fs.readFile(file)
        response.writeHead(200, { 'Content-Type': ({ '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css' })[path.extname(file)] ?? 'application/octet-stream' })
        response.end(content)
      } catch { response.writeHead(404); response.end() }
    },
  })
  t.after(() => server.close())
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext({ permissions: ['clipboard-read', 'clipboard-write'], acceptDownloads: true })
  const page = await context.newPage()
  page.setDefaultTimeout(10_000)
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.route('**/*', route => new URL(route.request().url()).origin === server.origin ? route.continue() : route.abort())
  await page.goto(server.origin)
  await page.locator('#workspace-token').fill('effects-fixture-token')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.locator('#workspace-gate[hidden]').waitFor({ state: 'attached' })
  await page.waitForFunction(() => !!window.kaminBridge)

  await page.evaluate(() => { window.__result = undefined; void window.kaminBridge.openFileDialog().then(value => { window.__result = value }) })
  await page.getByLabel('Attachments', { exact: true }).setInputFiles({ name: 'browser-upload.txt', mimeType: 'text/plain', buffer: Buffer.from('actual browser bytes') })
  await page.getByRole('button', { name: 'Upload', exact: true }).click()
  await page.waitForFunction(() => Array.isArray(window.__result))
  const uploaded = await page.evaluate(() => window.__result[0])
  assert.ok(uploaded.startsWith(path.join(root, '.bridge-uploads') + path.sep))
  assert.equal(await fs.readFile(uploaded, 'utf8'), 'actual browser bytes')

  await page.evaluate(() => { window.__result = undefined; void window.kaminBridge.writeClipboard('clipboard roundtrip').then(value => { window.__result = value }) })
  await page.getByRole('button', { name: 'Copy', exact: true }).click()
  await page.waitForFunction(() => window.__result === true)
  await page.evaluate(() => { window.__result = undefined; void window.kaminBridge.readClipboard().then(value => { window.__result = value }) })
  await page.getByRole('button', { name: 'Paste', exact: true }).click()
  await page.waitForFunction(() => window.__result === 'clipboard roundtrip')

  await page.evaluate(() => { window.__result = undefined; void window.kaminBridge.saveTextAs('real export bytes', 'browser-export.txt').then(value => { window.__result = value }) })
  const downloadReady = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Download', exact: true }).click()
  const download = await downloadReady
  assert.equal(download.suggestedFilename(), 'browser-export.txt')
  const chunks = []
  for await (const chunk of await download.createReadStream()) chunks.push(chunk)
  assert.equal(Buffer.concat(chunks).toString(), 'real export bytes')
  await page.waitForFunction(() => window.__result?.ok === true)

  await page.evaluate(() => { window.__result = undefined; void window.kaminBridge.readClipboard().catch(error => { window.__result = error.message }) })
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await page.waitForFunction(() => /cancelled/.test(window.__result))
  assert.deepEqual([...unexpected], [])
  assert.deepEqual(errors, [])
})
