import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { setTimeout as sleep } from 'node:timers/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { previewFixture } from './fixtures/preview-fixture.mjs'

// Synthetic upstream, real shared host, production browser bundle. A session
// whose transcript tail is mostly bookkeeping rows, re-delivered with fresh
// replay ords (as reattach/reconnect replays do), must still show its
// conversation, store no unread rows, and keep one copy of each read row.
test('chat keeps the conversation through a bookkeeping-heavy tail and repeated deliveries', { timeout: 60000 }, async t => {
  const fixture = await previewFixture(t, { conversations: true })
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  async function open(context) {
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
  const context = await browser.newContext()
  t.after(() => context.close())
  let page = await open(context)
  await page.getByRole('button', { name: 'No folder session', exact: true }).click()
  let tab
  const deadline = Date.now() + 15000
  while (!(tab = (await page.evaluate(() => window.kaminBridge.listTabs()))[0])?.conversationId) {
    assert.ok(Date.now() < deadline, 'Conversation identity timed out')
    await sleep(50)
  }
  let ord = 0
  const conversation = () => [
    { type: 'user', uuid: 'parity-user', _pos: 0, _ord: ++ord, timestamp: '2026-11-10T08:00:00Z',
      message: { role: 'user', content: 'PARITY_QUESTION' } },
    { type: 'assistant', uuid: 'parity-assistant', parentUuid: 'parity-user', _pos: 100, _ord: ++ord,
      timestamp: '2026-11-10T08:00:01Z',
      message: { id: 'parity-message', role: 'assistant', content: [{ type: 'text', text: 'PARITY_ANSWER' }] } },
    { type: 'mode', mode: 'default', _pos: 200, _ord: ++ord },
  ]
  fixture.send(tab.conversationId, { type: 'jsonl:entries', entries: conversation() })
  fixture.send(tab.conversationId, { type: 'jsonl:entries', entries: Array.from({ length: 3000 }, (_, n) =>
    ({ type: n % 2 ? 'cost-state' : 'atis-latch', _pos: 300 + n * 100, _ord: ++ord })) })
  for (let n = 0; n < 3; n++) fixture.send(tab.conversationId, { type: 'jsonl:entries', entries: conversation() })
  fixture.send(tab.conversationId, { type: 'jsonl:status', status: 'watching', replayComplete: true })

  const check = async label => {
    try {
      await page.getByText('PARITY_ANSWER', { exact: false }).waitFor()
    } catch (error) {
      t.diagnostic(JSON.stringify(await page.evaluate(async id => ({
        stores: Object.entries(window.__jsonlEntries ?? {}).map(([key, rows]) => ({ key, count: rows.length,
          types: rows.reduce((acc, row) => ({ ...acc, [row.type]: (acc[row.type] ?? 0) + 1 }), {}) })),
        tabs: await window.kaminBridge.listTabs?.(), id }), tab.id)))
      throw error
    }
    await page.waitForFunction(id => (window.__jsonlEntries?.[id] ?? []).some(row => row.type === 'mode'), tab.id)
    const types = await page.evaluate(id => window.__jsonlEntries[id].map(row => row.type), tab.id)
    const count = type => types.filter(value => value === type).length
    assert.equal(count('cost-state') + count('atis-latch'), 0, `${label}: unread rows must not be stored`)
    assert.equal(count('mode'), 1, `${label}: a re-delivered uuid-less row must be stored once`)
    assert.equal(count('user'), 1, `${label}: user row`)
    assert.equal(count('assistant'), 1, `${label}: assistant row`)
    assert.equal(await page.locator('.jsonl-entry-user').filter({ hasText: 'PARITY_QUESTION' }).count(), 1)
  }
  await check('live page')
  // A fresh page, with no other page open, gets the host's replay of the same rows.
  await context.close()
  const fresh = await browser.newContext()
  t.after(() => fresh.close())
  page = await open(fresh)
  await check('reloaded page')
})
