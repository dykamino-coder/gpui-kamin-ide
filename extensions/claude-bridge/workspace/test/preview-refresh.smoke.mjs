import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { previewFixture } from './fixtures/preview-fixture.mjs'
import { refreshRemote } from './fixtures/refresh-remote.mjs'

test('documented Rspack HMR path preserves refresh state and resets changed hook signatures', { timeout: 120000 }, async t => {
  const fixture = await previewFixture(t)
  const remote = await refreshRemote(t, fixture.root)
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  page.setDefaultTimeout(25000)
  const errors = [], diagnostics = []
  const sockets = []
  const hmrFrames = []
  page.on('pageerror', error => errors.push(error.message))
  page.on('console', message => diagnostics.push(message.text()))
  page.on('websocket', socket => {
    sockets.push(new URL(socket.url()).pathname)
    if (new URL(socket.url()).pathname === '/preview/remotes/refresh_demo/ws') {
      socket.on('framereceived', ({ payload }) => {
        try { hmrFrames.push(JSON.parse(String(payload)).type) } catch {}
      })
    }
  })
  await page.route('**/*', route => new URL(route.request().url()).origin === fixture.origin ? route.continue() : route.abort())
  await page.goto(fixture.origin)
  await page.locator('#workspace-token').fill('synthetic-preview-token')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.waitForFunction(() => !!window.bridgeCmd)
  await page.evaluate(async () => window.kaminBridge.createTab(await window.kaminBridge.getConfig()))
  await fixture.wait(frame => frame.type === 'session:create')
  await fixture.tool('preview_mount', remote)
  try {
    await page.getByRole('button', { name: 'Before 0', exact: true }).click()
    await page.getByRole('button', { name: 'Before 1', exact: true }).click()
    await page.getByRole('button', { name: 'Before 2', exact: true }).waitFor()
    await page.evaluate(() => { window.__refreshDocument = 'unchanged'; window.__refreshCommit = window.__REACT_DEVTOOLS_GLOBAL_HOOK__.onCommitFiberRoot })
    await remote.edit('After')
    await page.getByRole('button', { name: 'After 2', exact: true }).waitFor()
    await page.waitForTimeout(7000)
    await page.getByRole('button', { name: 'After 2', exact: true }).waitFor()
    assert.equal(await page.evaluate(() => window.__refreshDocument), 'unchanged')
    if (process.env.BRIDGE_BROWSER_SCREENSHOT) await page.screenshot({ path: process.env.BRIDGE_BROWSER_SCREENSHOT.replace(/\.png$/, '-refresh.png'), fullPage: true })
    await remote.edit('New hook', true)
    await page.getByRole('button', { name: 'New hook 0', exact: true }).waitFor()
    assert.equal(await page.evaluate(() => window.__refreshDocument), 'unchanged')
    assert.ok(sockets.includes('/preview/remotes/refresh_demo/ws'))
    assert.ok(hmrFrames.includes('hash') && hmrFrames.includes('ok'), 'The prefixed socket must complete its handshake and deliver dev-server builds')
    assert.ok(!sockets.includes('/ws'), 'The remote must not start an unproxied dev client')
    await fixture.tool('preview_mount', remote)
    await page.getByRole('button', { name: 'New hook 0', exact: true }).waitFor()
    assert.equal(await page.evaluate(() => window.__refreshCommit === window.__REACT_DEVTOOLS_GLOBAL_HOOK__.onCommitFiberRoot), true, 'Reloading a remote must not reinject another Refresh runtime')
    assert.equal(await page.evaluate(() => window.kaminBridge.getVersion()), 'fixture')
    await fixture.tool('preview_unmount', {})
    await page.getByRole('region', { name: 'Preview', exact: true }).waitFor({ state: 'hidden' })
    assert.deepEqual(errors.filter(error => error !== 'Unsupported workspace invoke: get-editor-selection'), [])
  } catch (error) {
    t.diagnostic(JSON.stringify({ errors, diagnostics, sockets, preview: await page.locator('.workspace-preview').innerText() }))
    throw error
  }
})
