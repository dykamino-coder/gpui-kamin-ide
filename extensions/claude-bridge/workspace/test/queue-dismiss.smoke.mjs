import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { setTimeout as sleep } from 'node:timers/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { previewFixture } from './fixtures/preview-fixture.mjs'

// Synthetic upstream; real shared BridgeHost, authenticated WS and production UI.
test('Escape restores fallback text once and host dismissal reaches other and reopened pages', { timeout: 60000 }, async t => {
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
  const text = 'Restore this queued message in the composer'
  await a.evaluate(({ id, text }) => window.kaminBridge.submitText(id, text, 'escape-restored'), { id: tab.id, text })
  await fixture.wait(frame => frame.type === 'session:submitText' && frame.submissionId === 'escape-restored')
  const fallback = { type: 'session:submission', id: 'escape-restored', state: 'fallback', cancelable: false }
  fixture.send(tab.conversationId, fallback)
  await Promise.all([queue(a).waitFor(), queue(b).waitFor()])
  // Stop is deliberately distinct: it interrupts without restoring queue text.
  await a.evaluate(id => window.kaminBridge.interruptSession(id), tab.id)
  await fixture.wait(frame => frame.type === 'session:interrupt')
  assert.equal(await queue(a).count(), 1)
  assert.equal(await queue(b).count(), 1)
  await a.locator('#prompt-input').focus()
  await a.locator('#prompt-input').press('Escape')
  await b.waitForFunction(() => window.queueReceipts.some(row => row.id === 'escape-restored' && row.fallbackQueue?.dismissed))
  await Promise.all([queue(a).waitFor({ state: 'hidden' }), queue(b).waitFor({ state: 'hidden' })])
  assert.equal(await a.locator('#prompt-input').inputValue(), text)
  await b.waitForFunction(expected => document.querySelector('#prompt-input')?.value === expected, text)
  await a.locator('[data-workspace-draft="saved"]').waitFor()
  await b.locator('[data-workspace-draft="saved"]').waitFor()
  await Promise.all([a.close(), b.close()])
  fixture.send(tab.conversationId, fallback) // Duplicate receipt with zero pages.
  const reopened = await open()
  await reopened.waitForFunction(() => window.queueReceipts.some(row => row.id === 'escape-restored' && row.fallbackQueue?.dismissed))
  assert.equal(await queue(reopened).count(), 0)
  await reopened.waitForFunction(expected => document.querySelector('#prompt-input')?.value === expected, text)
  await reopened.locator('[data-workspace-draft="saved"]').waitFor()
  const receipt = await reopened.evaluate(() => window.queueReceipts.find(row => row.id === 'escape-restored' && row.fallbackQueue?.dismissed))
  assert.equal(receipt.state, 'fallback', 'Host dismissal never claims CLI delivery was cancelled')
  assert.notEqual(receipt.fallbackQueue.consumed, true)
})
