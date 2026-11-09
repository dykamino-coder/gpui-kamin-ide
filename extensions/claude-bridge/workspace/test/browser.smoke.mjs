if (process.platform === 'win32') throw new Error('Windows host browsers are forbidden; use test:browser:container with --report-dir outside checkout')

// Real Chromium + HTTP/WS boundary + production browser bundle. This deliberate
// fixture peer proves browser startup/lifecycle, not shared-handler/model parity.
import assert from 'node:assert/strict'
import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { gzipSync, brotliCompressSync } from 'node:zlib'
import { chromium } from 'playwright'
import { WorkspaceAuth, startWorkspaceServer, createAppAssetHandler } from '../dist/index.js'
import './linux-browser-only.mjs'

test('browser signs in, mounts the shared App, rejects forged replies and resyncs without replay', { timeout: 60_000 }, async t => {
  const assets = fileURLToPath(new URL('../../webview/dist/browser/', import.meta.url))
  const files = new Map((await readdir(assets, { recursive: true, withFileTypes: true }))
    .filter(entry => entry.isFile()).map(entry => {
      const filename = path.join(entry.parentPath, entry.name)
      return ['/' + path.relative(assets, filename).split(path.sep).join('/'), filename]
    }))
  const scripts = await Promise.all([...files.values()].filter(filename => filename.endsWith('.js')).map(filename => readFile(filename)))
  t.diagnostic(JSON.stringify({ measurement: 'production-js-bundle', chunks: scripts.length,
    rawBytes: scripts.reduce((sum, bytes) => sum + bytes.length, 0),
    gzipBytes: scripts.reduce((sum, bytes) => sum + gzipSync(bytes).length, 0),
    brotliBytes: scripts.reduce((sum, bytes) => sum + brotliCompressSync(bytes).length, 0) }))
  let valid = true
  let attachments = 0
  const calls = []
  const unexpected = new Set()
  let held
  let holdSettingsSave = false
  let settingsSaveRead
  const settingsSavePending = new Promise(resolve => { settingsSaveRead = resolve })
  const config = { serverUrl: 'ws://fixture.invalid', token: '[workspace credential]', cwd: '/workspace' }
  const replies = new Map([
    ['get-version', 'fixture'], ['get-config', config],
    ['layout:get', { filePanelVisible: true, filePanelBottomVisible: false }],
    ['skills:list', []], ['session:get-saved', []], ['tab:list', []], ['tabs:restore-state', []],
    ['vscode:is-available', false], ['mcp:status', []], ['mic-connected', false],
    ['get-editor-selection', null], ['bridge:server-version', null],
    ['get-machine-id', 'browser-fixture-machine'], ['hooks:list-pending-plugin-approvals', []],
    ['base-prompt:get', ''], ['base-prompt:set', true],
    ['bridge:server-fetch', { ok: false, status: 404, data: null }],
    ['notifications:get', {}], ['terminal:get-prefs', {}],
  ])
  const server = await startWorkspaceServer({
    port: 0, revalidateMs: 500,
    auth: new WorkspaceAuth('browser-fixture-token', async () => valid ? { id: 'browser-fixture' } : null),
    backend: { attach(view) {
      attachments++
      return {
        async receive(frame) {
          if (frame.kind !== 'invoke') return
          calls.push(frame)
          if (frame.channel === 'get-config' && holdSettingsSave) { settingsSaveRead(); return }
          if (frame.channel === 'read-clipboard') { held = frame; return }
          if (frame.channel === 'logs:clear') throw new Error('Fixture disconnect')
          const known = replies.has(frame.channel)
          if (!known) unexpected.add(frame.channel)
          await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation,
            ok: known, ...(known ? { result: replies.get(frame.channel) } : { error: 'Unsupported fixture channel' }) })
        },
        dispose() {},
      }
    } },
    serveApp: await createAppAssetHandler(assets),
  })
  t.after(() => server.close())
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  const cdp = await page.context().newCDPSession(page)
  async function reportHeap(stage) {
    await cdp.send('HeapProfiler.collectGarbage')
    const heap = await cdp.send('Runtime.getHeapUsage')
    t.diagnostic(JSON.stringify({ measurement: 'fixture-page-js-heap', stage, ...heap }))
  }
  page.setDefaultTimeout(10_000)
  const errors = []
  page.on('pageerror', error => errors.push(error.stack))
  // Keep acceptance deterministic/offline; no remote fonts or external links.
  await page.route('**/*', route => new URL(route.request().url()).origin === server.origin ? route.continue() : route.abort())
  await page.goto(server.origin)
  await page.locator('#workspace-login:not([hidden])').waitFor()
  assert.equal(attachments, 0)
  assert.equal(await page.evaluate(() => typeof window.kaminBridge), 'undefined')
  await page.locator('#workspace-token').fill('wrong-fixture-token')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('status').filter({ hasText: 'Sign-in failed.' }).waitFor()
  assert.equal(await page.locator('#workspace-token').inputValue(), '')
  assert.equal(attachments, 0)
  async function signIn() {
    await page.locator('#workspace-token').fill('browser-fixture-token')
    await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    await page.locator('#app-wrapper').waitFor()
    await page.locator('#workspace-gate[hidden]').waitFor({ state: 'attached' })
    await page.getByText('No sessions', { exact: true }).waitFor()
  }
  await signIn()
  await reportHeap('initial-shared-App')
  assert.equal(attachments, 1)
  await page.locator('.file-panel-column').waitFor()
  // Synthetic fixture UI only; local captures must use a private destination.
  if (process.env.BRIDGE_BROWSER_SCREENSHOT) await page.screenshot({ path: process.env.BRIDGE_BROWSER_SCREENSHOT })
  assert.equal(await page.locator('iframe').count(), 0)
  assert.equal(await page.evaluate(() => localStorage.length + sessionStorage.length), 0)
  assert.equal((await page.context().cookies()).find(cookie => cookie.name.includes('workspace'))?.httpOnly, true)

  // Both sidebar/settings are the existing App components, not a test-only UI.
  await page.getByRole('button', { name: 'Customize', exact: true }).click()
  await page.locator('#sidebar-settings-item').waitFor()
  await page.locator('#sidebar-mcp-item').waitFor()
  await page.waitForFunction(() => [...document.querySelectorAll('input')].some(input => input.value === 'ws://fixture.invalid'))
  // Keep the debounced settings save's config read pending across disconnect.
  // Its failed send must be reported without an unhandled rejection or replay.
  holdSettingsSave = true
  await settingsSavePending
  holdSettingsSave = false
  await page.evaluate(() => { window.__workspaceDocument = 'same-document' })
  await page.evaluate(() => window.kaminBridge.setBasePrompt('one fixture mutation'))
  await page.evaluate(() => {
    window.__probe = 'pending'
    void window.kaminBridge.readClipboard().then(() => { window.__probe = 'forged' }, error => { window.__probe = error.message })
  })
  await page.waitForFunction(() => window.kaminBridge !== undefined)
  // A second request/reply ensures the held call has reached the peer.
  await page.evaluate(() => window.kaminBridge.getConfig())
  assert.ok(held)
  await page.evaluate(frame => window.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result: 'forged' }, '*'), held)
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  assert.equal(await page.evaluate(() => window.__probe), 'pending')
  const listsBefore = calls.filter(frame => frame.channel === 'tab:list').length
  const closeError = await page.evaluate(() => window.kaminBridge.clearLogs().then(() => '', error => error.message))
  assert.match(closeError, /outcome may be unknown/)
  await page.waitForFunction(() => document.querySelector('#workspace-gate')?.hidden === true && /outcome may be unknown/.test(window.__probe))
  await page.evaluate(() => window.kaminBridge.getConfig())
  assert.equal(attachments, 2)
  assert.equal(await page.evaluate(() => window.__workspaceDocument), 'same-document')
  assert.equal(calls.filter(frame => frame.channel === 'base-prompt:set').length, 1)
  assert.equal(calls.filter(frame => frame.channel === 'read-clipboard').length, 1)
  assert.equal(calls.filter(frame => frame.channel === 'set-config').length, 0)
  await page.getByText('Settings could not be confirmed. Check the connection and try again.', { exact: true }).waitFor()
  assert.ok(calls.filter(frame => frame.channel === 'tab:list').length > listsBefore)
  await reportHeap('after-socket-reattach')

  valid = false
  await page.locator('#workspace-login:not([hidden])').waitFor()
  assert.equal(await page.locator('#root').getAttribute('inert'), '')
  valid = true
  await signIn()
  await page.getByRole('button', { name: 'Sign out', exact: true }).click()
  await page.locator('#workspace-login:not([hidden])').waitFor()
  assert.equal(await page.evaluate(() => typeof window.kaminBridge), 'undefined')
  assert.deepEqual([...unexpected], [], 'Fixture must enumerate all observed invokes; unknown calls never succeed')
  assert.deepEqual(errors, [], 'Browser must have no unhandled renderer errors')
})
