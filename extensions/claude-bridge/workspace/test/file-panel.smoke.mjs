import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { WorkspaceAuth, WorkspaceFiles, startWorkspaceServer, createAppAssetHandler } from '../dist/index.js'
import { filePanelPeer } from './file-panel-peer.mjs'

test('production FilePanel recovers host controls across zero pages and reads current disk without saving', { timeout: 60_000 }, async t => {
  const root = await mkdtemp(join(tmpdir(), 'bridgeweb-panel-'))
  t.after(async () => {
    assert.equal(dirname(resolve(root)), resolve(tmpdir()))
    assert.ok(basename(root).startsWith('bridgeweb-panel-'))
    await rm(root, { recursive: true, force: true })
  })
  const a = join(root, 'a.txt'), b = join(root, 'b.html'), other = join(root, 'other.txt')
  const original = 'first line\nsecond line\nthird line\n'
  await writeFile(a, original); await writeFile(b, '<b>preview fixture</b>'); await writeFile(other, 'other chat')
  const peer = filePanelPeer(root), watchErrors = []
  const files = new WorkspaceFiles(peer.backend, { root, onWatchError: error => watchErrors.push(error.message) })
  t.after(() => files.dispose())
  const seed = files.attach({ async postMessage() { return true } })
  await seed.receive({ kind: 'send', channel: 'workspace:file-panel', args: [{ version: 1, tabs: {
    one: { files: [b, a], active: a, selection: { path: a, startLine: 2, endLine: 2, text: 'second', anchor: 11, head: 17 } },
    two: { files: [other], active: other, selection: null },
  }, previewModes: [[b, 'preview']], attachActiveFile: true }] })
  seed.dispose()
  const mutations = [], controls = []
  const backend = { attach(view) {
    const child = files.attach({ postMessage(frame) {
      if (frame.channel === 'workspace:files-state') controls.push({ direction: 'host', panel: frame.args[0]?.panel })
      return view.postMessage(frame)
    } })
    return { receive(frame) {
      if (frame.kind === 'invoke' && ['file-viewer:write', 'file-tree:rename'].includes(frame.channel)) mutations.push(frame.channel)
      if (frame.channel === 'workspace:file-panel') controls.push({ direction: 'page', panel: frame.args[0] })
      return child.receive(frame)
    }, dispose() { child.dispose() } }
  } }
  const server = await startWorkspaceServer({ port: 0,
    auth: new WorkspaceAuth('synthetic-panel-token', async () => ({ id: 'fixture' })), backend,
    serveApp: await createAppAssetHandler(fileURLToPath(new URL('../../webview/dist/browser/', import.meta.url))) })
  t.after(() => server.close())
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  await context.route('**/*', route => new URL(route.request().url()).origin === server.origin ? route.continue() : route.abort())
  const errors = []
  context.on('page', page => { page.setDefaultTimeout(10_000); page.on('pageerror', e => errors.push(e.message)) })
  async function open() {
    const page = await context.newPage()
    await page.goto(server.origin)
    await page.waitForFunction(() => !document.querySelector('#workspace-login')?.hidden || !!window.kaminBridge)
    if (await page.locator('#workspace-login:not([hidden])').count()) {
      await page.locator('#workspace-token').fill('synthetic-panel-token')
      await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    }
    await page.locator('.file-panel-column .cm-content').filter({ hasText: /first|short|other/ }).waitFor()
    return page
  }
  const strip = page => page.locator('.file-panel-column .conv-seg-strip > [data-tooltip]')
  const editor = page => page.locator('.file-panel-column .cm-content:visible')
  const selected = page => page.evaluate(() => window.getSelection()?.toString())
  // The chat input also autofocuses on tab startup. Focus the editor without
  // clicking/collapsing its range before observing the native DOM selection.
  const restoredSelection = async (page, text) => {
    try {
      await page.waitForFunction(expected => {
        document.querySelector('.file-panel-column .cm-content')?.focus()
        return window.getSelection()?.toString() === expected
      }, text)
    } catch (error) {
      t.diagnostic(JSON.stringify({ expected: text, selected: await selected(page), controls: controls.slice(-12) }))
      throw error
    }
  }
  let page = await open()
  assert.deepEqual(await strip(page).evaluateAll(nodes => nodes.map(n => n.getAttribute('data-tooltip'))), [b, a])
  await restoredSelection(page, 'second')
  assert.equal(await page.locator('.attach-file-switch').getAttribute('aria-pressed'), 'true')
  // Change the real CodeMirror selection with the keyboard. The host must
  // retain direction and exact offsets, not just the seed or watch order.
  await editor(page).press('Control+Home')
  await editor(page).press('Shift+End')
  await restoredSelection(page, 'first line')
  // Read-only invoke orders after the immediate control send on this socket.
  await page.evaluate(() => window.kaminBridge.getConfig())
  await page.close()
  assert.equal(context.pages().length, 0)
  await new Promise(resolve => setTimeout(resolve, 100))
  assert.equal(peer.attached, 0)
  page = await open()
  await restoredSelection(page, 'first line')
  assert.deepEqual(await strip(page).evaluateAll(nodes => nodes.map(n => n.getAttribute('data-tooltip'))), [b, a])
  assert.equal(await page.locator('.attach-file-switch').getAttribute('aria-pressed'), 'true')
  await strip(page).filter({ hasText: 'b.html' }).click()
  // This assertion covers the retained mode only. Preview rendering/HMR has
  // separate acceptance; the legacy guest element is not a browser renderer.
  await page.locator('.file-panel-column :is(webview,iframe)').waitFor({ state: 'attached' })
  assert.equal(await editor(page).count(), 0)
  // Explicitly closed files must stay closed even while the host watches them.
  await strip(page).filter({ hasText: 'b.html' }).getByRole('button', { name: 'Close file' }).click()
  await editor(page).filter({ hasText: 'first line' }).waitFor()
  await page.locator('.attach-file-switch').click()
  await editor(page).press('Control+End')
  await page.evaluate(() => window.kaminBridge.getConfig())
  await page.close()
  await new Promise(resolve => setTimeout(resolve, 100))
  assert.equal(peer.attached, 0)
  // Agent edits current disk with no browser. Reopen clamps the retained caret
  // to the new shorter document instead of restoring/saving stale bytes.
  await writeFile(a, 'short')
  page = await open()
  assert.equal(await editor(page).innerText(), 'short')
  assert.deepEqual(await strip(page).evaluateAll(nodes => nodes.map(n => n.getAttribute('data-tooltip'))), [a])
  assert.equal(await page.locator('.attach-file-switch').getAttribute('aria-pressed'), 'false')
  assert.equal(await selected(page), '')
  await editor(page).press('Shift+Home')
  await page.waitForFunction(() => window.getSelection()?.toString() === 'short')
  await page.evaluate(() => window.kaminBridge.switchTab('two'))
  await editor(page).filter({ hasText: 'other chat' }).waitFor()
  assert.deepEqual(await strip(page).evaluateAll(nodes => nodes.map(n => n.getAttribute('data-tooltip'))), [other])
  await page.evaluate(() => window.kaminBridge.switchTab('one'))
  await editor(page).filter({ hasText: 'short' }).waitFor()
  // Second page gets its own host projection and may close its file without
  // changing the first page's controls or its remembered view identity.
  const second = await open()
  await strip(second).getByRole('button', { name: 'Close file' }).click()
  await second.evaluate(() => window.kaminBridge.getConfig())
  assert.equal(await strip(page).count(), 1)
  await page.reload()
  await editor(page).filter({ hasText: 'short' }).waitFor()
  assert.equal(await strip(page).count(), 1)
  assert.equal(await readFile(a, 'utf8'), 'short')
  // CRLF disk bytes normalize to shorter CodeMirror text. Clamp against that
  // document, not the raw string length, including a reversed selection.
  await writeFile(a, 'x\r\n')
  await page.reload()
  await editor(page).locator('.cm-line').first().filter({ hasText: /^x$/ }).waitFor()
  assert.equal(await readFile(a, 'utf8'), 'x\r\n')
  assert.deepEqual(mutations, [])
  assert.deepEqual([...peer.unexpected], [])
  assert.deepEqual(watchErrors, [])
  assert.deepEqual(errors, [])
  t.diagnostic('Synthetic session peer; production App, HTTP/WS, FilePanel, CodeMirror and container file adapter. No model/live-server parity claim.')
})
