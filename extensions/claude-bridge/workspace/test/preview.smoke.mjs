import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { previewFixture, remoteFixture } from './fixtures/preview-fixture.mjs'

// The agent peer is synthetic. MCP transport, shared tool handlers, host state,
// authenticated proxy, MF runtime, webpack HMR and Chromium are real.
test('agent MCP mounts MF next to chat, HMR keeps the root, reopen restores and errors stay in panel', { timeout: 120000 }, async t => {
  const fixture = await previewFixture(t)
  const remote = await remoteFixture(t, fixture.root)
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  let page = await context.newPage()
  const errors = []
  const diagnostics = []
  page.on('console', message => { if (/webpack|HMR|Preview/i.test(message.text())) diagnostics.push(message.text()) })
  page.on('websocket', socket => diagnostics.push(`WS ${new URL(socket.url()).pathname}`))
  page.on('pageerror', error => errors.push(error.message))
  page.setDefaultTimeout(20000)
  await page.route('**/*', route => new URL(route.request().url()).origin === fixture.origin ? route.continue() : route.abort())
  await page.goto(fixture.origin)
  await page.locator('#workspace-token').fill('synthetic-preview-token')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.waitForFunction(() => !!window.bridgeCmd)
  const tab = await page.evaluate(async () => window.kaminBridge.createTab(await window.kaminBridge.getConfig()))
  await fixture.wait(frame => frame.type === 'session:create')
  // Same public submit path as the chat input. Peer models the agent's response.
  await page.evaluate(tabId => window.kaminBridge.submitText(tabId, 'Show my micro-frontend', 'preview-chat-request'), tab)
  await fixture.wait(frame => frame.type === 'session:submitText' && frame.submissionId === 'preview-chat-request')
  await fixture.tool('preview_mount', { name: remote.name, entry: remote.entry, exposedModule: remote.exposedModule, props: { label: 'one' } })
  await page.getByRole('button', { name: 'Before one 0', exact: true }).click()
  await page.getByRole('button', { name: 'Before one 1', exact: true }).waitFor()
  assert.equal(await page.locator('iframe').count(), 0)
  assert.equal(await page.getByRole('region', { name: 'Preview', exact: true }).locator('input').count(), 0)
  await page.evaluate(() => { window.__previewDocument = 'same-document' })
  await remote.edit('After')
  try { await page.getByRole('button', { name: 'After one 0', exact: true }).waitFor() }
  catch (error) { t.diagnostic(JSON.stringify({ diagnostics, errors, preview: await page.locator('.workspace-preview').innerText() })); throw error }
  // Wait beyond the fallback window: real HMR must not later remount the root.
  await page.waitForTimeout(6500)
  assert.equal(await page.evaluate(() => window.__previewMounts), 1)
  assert.equal(await page.evaluate(() => window.__previewDocument), 'same-document')
  assert.equal(await page.evaluate(() => window.kaminBridge.getVersion()), 'fixture')
  await page.evaluate(tabId => window.kaminBridge.submitText(tabId, 'Keep working while preview updates', 'preview-chat-after-hmr'), tab)
  await fixture.wait(frame => frame.type === 'session:submitText' && frame.submissionId === 'preview-chat-after-hmr')
  if (process.env.BRIDGE_BROWSER_SCREENSHOT) await page.screenshot({
    path: process.env.BRIDGE_BROWSER_SCREENSHOT.replace(/\.png$/, '-preview.png'), fullPage: true,
  })
  await fixture.tool('preview_update', { props: { label: 'two' } })
  await page.getByRole('button', { name: 'After two 0', exact: true }).waitFor()
  await page.close()
  // The tool still acts with zero pages; the host owns the descriptor and props.
  await fixture.tool('preview_update', { props: { label: 'reopened' } })
  page = await context.newPage()
  page.setDefaultTimeout(20000)
  await page.goto(fixture.origin)
  await page.getByRole('button', { name: 'After reopened 0', exact: true }).waitFor()
  await fixture.tool('preview_mount', { name: remote.name, entry: remote.entry, exposedModule: './Component', props: { label: 'react' } })
  await page.getByRole('button', { name: 'After react 0', exact: true }).waitFor()
  await fixture.tool('preview_update', { props: { fail: true } })
  await page.getByRole('status').filter({ hasText: 'Fixture render failure' }).waitFor()
  assert.equal(await page.evaluate(() => window.kaminBridge.getVersion()), 'fixture')
  const rebuild = await remoteFixture(t, fixture.root, { name: 'rebuild_fixture', hot: false })
  await fixture.tool('preview_mount', { name: rebuild.name, entry: rebuild.entry, exposedModule: './Component', props: { label: 'fallback' } })
  await page.getByRole('button', { name: 'Before fallback 0', exact: true }).waitFor()
  await page.evaluate(() => { window.__previewDocument = 'rebuild-document' })
  await rebuild.edit('Rebuilt')
  await page.getByRole('button', { name: 'Rebuilt fallback 0', exact: true }).waitFor()
  assert.equal(await page.evaluate(() => window.__previewDocument), 'rebuild-document')
  const mismatch = await remoteFixture(t, fixture.root, { name: 'version_mismatch', requiredVersion: '99.0.0', hot: false })
  await fixture.tool('preview_mount', { name: mismatch.name, entry: mismatch.entry, exposedModule: './Component' })
  await page.getByRole('status').filter({ hasText: /Unsatisfied version|version.*19\.3\.1|19\.3\.1.*version/i }).waitFor()
  assert.equal(await page.evaluate(() => window.kaminBridge.getVersion()), 'fixture')
  await fixture.tool('preview_unmount', {})
  await page.getByRole('region', { name: 'Preview', exact: true }).waitFor({ state: 'hidden' })
  // The native IDE editor-context channel is explicitly outside the browser
  // host contract; report it separately from failures introduced by preview.
  const nativeEditor = 'Unsupported workspace invoke: get-editor-selection'
  if (errors.includes(nativeEditor)) t.diagnostic('Known chat startup issue: native IDE editor-selection invoke is unsupported')
  assert.deepEqual(errors.filter(error => error !== nativeEditor), [], 'Initial remote/HMR must not cause unhandled errors')
})
