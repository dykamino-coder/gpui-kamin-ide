import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'
import { gzipSync, brotliCompressSync } from 'node:zlib'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { heap } from '../acceptance/browser.mjs'
import { previewFixture } from './fixtures/preview-fixture.mjs'

// Синтетический upstream; настоящий BridgeHost, auth/WS и production UI.
// Это page-JS regression budget, а не receipt полного actual-model контракта.
test('long transcript stays bounded through zero-page output and five fresh reattachments', { timeout: 120000 }, async t => {
  const assets = fileURLToPath(new URL('../../webview/dist/browser/', import.meta.url))
  const scripts = await Promise.all((await readdir(assets, { recursive: true, withFileTypes: true }))
    .filter(entry => entry.isFile() && entry.name.endsWith('.js'))
    .map(entry => readFile(path.join(entry.parentPath, entry.name))))
  t.diagnostic(JSON.stringify({ measurement: 'retention-production-js-bundle', chunks: scripts.length,
    rawBytes: scripts.reduce((sum, bytes) => sum + bytes.length, 0),
    gzipBytes: scripts.reduce((sum, bytes) => sum + gzipSync(bytes).length, 0),
    brotliBytes: scripts.reduce((sum, bytes) => sum + brotliCompressSync(bytes).length, 0) }))
  const fixture = await previewFixture(t, { conversations: true, holdSync: true })
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } })
  const errors = []
  let nativeEditorErrors = 0
  context.on('page', page => page.on('pageerror', error => {
    // Тот же известный native-only startup invoke, что в preview smoke.
    if (error.message === 'Unsupported workspace invoke: get-editor-selection') nativeEditorErrors++
    else errors.push('UNEXPECTED_PAGE_ERROR')
  }))
  await context.route('**/*', route => new URL(route.request().url()).origin === fixture.origin
    ? route.continue() : route.abort())
  t.diagnostic(JSON.stringify({ measurement: 'retention-environment', browser: browser.version(),
    source: process.env.BRIDGEWEB_ACC_SOURCE, sourceDirty: process.env.BRIDGEWEB_ACC_SOURCE_DIRTY,
    upstream: 'synthetic', host: 'shared BridgeHost', platform: process.platform }))
  async function open() {
    const page = await context.newPage()
    page.setDefaultTimeout(20000)
    await page.goto(fixture.origin)
    if (await page.locator('#workspace-token').isVisible()) {
      await page.locator('#workspace-token').fill('synthetic-preview-token')
      await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    }
    await page.getByRole('tablist', { name: 'Open sessions', exact: true }).waitFor()
    return page
  }
  const samples = []
  async function measure(page, stage) {
    const sample = { measurement: 'retention-page-js-heap', stage, ...await heap(context, page) }
    samples.push(sample)
    t.diagnostic(JSON.stringify(sample))
    assert.ok(sample.usedBytes < 128 * 1024 * 1024, `${stage}: post-GC page heap exceeds 128 MiB`)
  }
  let page = await open()
  await measure(page, 'startup')
  await page.getByRole('button', { name: 'No folder session', exact: true }).click()
  await fixture.waitForSync()
  await page.evaluate(async () => {
    const id = (await window.kaminBridge.listTabs())[0].id
    window.kaminBridge.switchTab(id)
    window.kaminBridge.switchTab(id)
    await window.kaminBridge.getActiveTab() // Same-socket dispatch barrier.
  })
  assert.equal(fixture.receivedFrames().filter(frame => ['session:create', 'session:resume'].includes(frame.type)).length,
    0, 'Activation must wait for initial user/project sync')
  await fixture.releaseSync()
  let tab
  const deadline = Date.now() + 20000
  while (!(tab = (await page.evaluate(() => window.kaminBridge.listTabs()))[0])?.conversationId) {
    assert.ok(Date.now() < deadline, 'Shared session identity timed out')
    await sleep(50)
  }
  const text = 'Synthetic long-turn retention workload'
  await page.evaluate(({ id, text }) => window.kaminBridge.submitText(id, text, 'retention-submit'), { id: tab.id, text })
  await fixture.wait(frame => frame.type === 'session:submitText' && frame.submissionId === 'retention-submit')
  fixture.send(tab.conversationId, { type: 'session:submission', id: 'retention-submit', state: 'fallback', cancelable: false })
  fixture.send(tab.conversationId, { type: 'jsonl:status', status: 'watching', replayComplete: true })
  fixture.send(tab.conversationId, { type: 'session:activity', isWorking: true, hookDriven: true, promptReady: false })
  const row = n => ({ type: n === 1 ? 'user' : 'assistant', uuid: `retention-${n}`, _ord: n,
    parentUuid: n === 1 ? null : `retention-${n - 1}`, timestamp: '2026-11-10T00:00:00Z',
    message: { id: `retention-message-${n}`, role: n === 1 ? 'user' : 'assistant',
      content: n === 1 ? text : [{ type: 'text', text: `Record ${n}: ${'bounded prose '.repeat(80)}` }] } })
  async function sendRange(from, to) {
    for (let start = from; start <= to; start += 250) {
      fixture.send(tab.conversationId, { type: 'jsonl:entries', entries:
        Array.from({ length: Math.min(250, to - start + 1) }, (_, i) => row(start + i)) })
      await sleep(10)
    }
  }
  async function tail(page, newest, stage, minimum = 4000, maximum = 4500) {
    // Ждём размер проекции, а не только появление последнего UUID.
    await page.waitForFunction(({ id, newest, minimum, maximum }) => {
      const entries = window.__jsonlEntries?.[id] ?? []
      return entries.at(-1)?.uuid === `retention-${newest}` && entries.length >= minimum && entries.length <= maximum
    }, { id: tab.id, newest, minimum, maximum })
    const snapshot = await page.evaluate(id => {
      const rows = window.__jsonlEntries[id]
      return { count: rows.length, ids: rows.map(row => row.uuid), ords: rows.map(row => row._ord),
        mounted: document.querySelectorAll('.jsonl-entry').length }
    }, tab.id)
    assert.ok(snapshot.count >= minimum && snapshot.count <= maximum, `${stage}: resident tail bounds`)
    assert.equal(new Set(snapshot.ids).size, snapshot.count, `${stage}: duplicate UUIDs`)
    assert.deepEqual(snapshot.ords, Array.from({ length: snapshot.count }, (_, i) => newest - snapshot.count + i + 1),
      `${stage}: contiguous newest tail survives reattachment`)
    assert.ok(snapshot.mounted > 0 && snapshot.mounted <= 150, `${stage}: mounted row window`)
    t.diagnostic(JSON.stringify({ measurement: 'retention-tail', stage, resident: snapshot.count, mounted: snapshot.mounted }))
    await measure(page, stage)
  }
  await sendRange(1, 8000)
  await tail(page, 8000, 'attached-8000')
  await page.close()
  assert.equal(context.pages().length, 0)
  await sendRange(8001, 10000)
  // Ответ общего tool-handler доказывает обработку предыдущих кадров без страницы.
  const response = await fixture.tool('Bash', { command: 'printf retention-zero-page', timeout: 1000 })
  assert.match(JSON.stringify(response.result), /retention-zero-page/, 'Shared Bash must return its real stdout')
  fixture.send(tab.conversationId, { type: 'session:activity', isWorking: false, hookDriven: true, promptReady: true })
  for (let cycle = 1; cycle <= 5; cycle++) {
    page = await open()
    await tail(page, 10000, `reattach-${cycle}`, 200, 200)
    await page.close()
    assert.equal(context.pages().length, 0)
  }
  const reattached = samples.filter(sample => sample.stage.startsWith('reattach-')).map(sample => sample.usedBytes)
  const spread = Math.max(...reattached) - Math.min(...reattached)
  t.diagnostic(JSON.stringify({ measurement: 'retention-reattach-spread', cycles: reattached.length, usedBytes: spread }))
  assert.ok(spread <= 16 * 1024 * 1024, 'Equal-tail fresh-page post-GC heaps span more than 16 MiB')
  const frames = fixture.receivedFrames()
  assert.equal(frames.filter(frame => frame.type === 'session:submitText').length, 1, 'Reattach must not resubmit')
  assert.equal(frames.filter(frame => frame.type === 'session:interrupt').length, 0, 'Detach must not interrupt')
  assert.equal(frames.filter(frame => ['session:create', 'session:resume'].includes(frame.type)).length, 1,
    'Reattach must reuse the live host session')
  t.diagnostic(JSON.stringify({ measurement: 'retention-known-native-editor-startup', count: nativeEditorErrors }))
  assert.deepEqual(errors, [])
})
