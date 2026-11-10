import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { setTimeout as sleep } from 'node:timers/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { previewFixture } from './fixtures/preview-fixture.mjs'

// Синтетический upstream; реальные shared BridgeHost, auth WS и production UI.
test('Remove dismisses one host queue item across pages and reconnect without interrupting CLI', { timeout: 60000 }, async t => {
  const fixture = await previewFixture(t, { conversations: true })
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } })
  await context.addInitScript(() => {
    window.queueReceipts = []
    const Socket = window.WebSocket
    window.WebSocket = class extends Socket {
      constructor(...args) {
        super(...args)
        this.addEventListener('message', event => {
          const frame = JSON.parse(event.data)
          if (frame.channel === 'session:submission') window.queueReceipts.push(frame.args[1])
        })
      }
    }
  })
  async function open() {
    const page = await context.newPage()
    page.setDefaultTimeout(15000)
    await page.goto(fixture.origin)
    if (await page.locator('#workspace-token').isVisible()) {
      await page.locator('#workspace-token').fill('synthetic-preview-token')
      await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    }
    await page.getByRole('tablist', { name: 'Open sessions', exact: true }).waitFor()
    return page
  }
  const a = await open()
  await a.getByRole('button', { name: 'No folder session', exact: true }).click()
  let tab
  const deadline = Date.now() + 15000
  while (!(tab = (await a.evaluate(() => window.kaminBridge.listTabs()))[0])?.conversationId) {
    assert.ok(Date.now() < deadline, 'Shared host session identity timed out')
    await sleep(50)
  }
  const b = await open()
  const queue = page => page.locator('.widget-card').filter({ has: page.getByText('Message Queue', { exact: true }) })
  for (const [id, text] of [['remove-first', 'Remove only this message'], ['keep-second', 'Keep this message queued']]) {
    await a.evaluate(({ tabId, text, id }) => window.kaminBridge.submitText(tabId, text, id), { tabId: tab.id, text, id })
    await fixture.wait(frame => frame.type === 'session:submitText' && frame.submissionId === id)
    fixture.send(tab.conversationId, { type: 'session:submission', id, state: 'fallback', cancelable: false })
  }
  await Promise.all([queue(a).getByText('2 pending', { exact: true }).waitFor(), queue(b).getByText('2 pending', { exact: true }).waitFor()])
  const before = fixture.receivedFrames().filter(frame => ['session:submitText', 'session:interrupt'].includes(frame.type))
  for (const [tabId, ids] of [['missing-tab', ['remove-first']], [tab.id, ['remove-first', 42]]]) {
    assert.equal(await a.evaluate(async ({ tabId, ids }) => {
      try { await window.kaminBridge.invoke('workspace:queue-dismiss', tabId, ids); return false }
      catch { return true }
    }, { tabId, ids }), true, 'Invalid removal must reject through the real invoke route')
  }
  assert.equal(await queue(b).getByRole('button', { name: 'Remove from queue', exact: true }).count(), 2)
  await queue(a).getByRole('button', { name: 'Remove from queue', exact: true }).first().click()
  await b.waitForFunction(() => window.queueReceipts.some(row => row.id === 'remove-first' && row.fallbackQueue?.dismissed))
  for (const page of [a, b]) {
    await queue(page).getByText('1 pending', { exact: true }).waitFor()
    assert.equal(await queue(page).getByText('Remove only this message', { exact: true }).count(), 0)
    await queue(page).getByText('Keep this message queued', { exact: true }).waitFor()
    assert.equal(await page.locator('#prompt-input').inputValue(), '')
  }
  await Promise.all([a.close(), b.close()])
  fixture.send(tab.conversationId, { type: 'session:submission', id: 'remove-first', state: 'fallback', cancelable: false })
  // Ответ следующего tool call подтверждает обработку receipt до открытия страницы.
  await fixture.tool('Glob', { pattern: '*.queue-remove-absent', path: fixture.root })
  const reopened = await open()
  await reopened.waitForFunction(() => window.queueReceipts.some(row => row.id === 'remove-first' && row.fallbackQueue?.dismissed))
  await queue(reopened).getByText('1 pending', { exact: true }).waitFor()
  assert.equal(await queue(reopened).getByText('Remove only this message', { exact: true }).count(), 0)
  await queue(reopened).getByText('Keep this message queued', { exact: true }).waitFor()
  assert.equal(await reopened.locator('#prompt-input').inputValue(), '')
  assert.deepEqual(fixture.receivedFrames().filter(frame => ['session:submitText', 'session:interrupt'].includes(frame.type)), before,
    'Remove and reconnect must not interrupt or resubmit text upstream')
  const receipt = await reopened.evaluate(() => window.queueReceipts.find(row => row.id === 'remove-first' && row.fallbackQueue?.dismissed))
  assert.equal(receipt.state, 'fallback')
  assert.notEqual(receipt.fallbackQueue.consumed, true)
})
