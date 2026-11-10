import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import { setTimeout as sleep } from 'node:timers/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { previewFixture } from './fixtures/preview-fixture.mjs'

test('crashed host restores uncertain and committed receipts to fresh pages without repeating submissions', { timeout: 60000 }, async t => {
  const fixture = await previewFixture(t, { conversations: true })
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  let context = await browser.newContext()
  async function open() {
    const page = await context.newPage()
    page.setDefaultTimeout(15000)
    const receipts = []
    page.on('websocket', socket => socket.on('framereceived', event => {
      const frame = JSON.parse(event.payload.toString())
      if (frame.channel === 'session:submission') receipts.push(frame.args[1])
    }))
    await page.goto(fixture.origin)
    if (await page.locator('#workspace-token').isVisible()) {
      await page.locator('#workspace-token').fill('synthetic-preview-token')
      await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    }
    await page.getByRole('tablist', { name: 'Open sessions', exact: true }).waitFor()
    return { page, receipts }
  }
  let { page } = await open()
  await page.getByRole('button', { name: 'No folder session', exact: true }).click()
  let tab
  const deadline = Date.now() + 15000
  while (!(tab = (await page.evaluate(() => window.kaminBridge.listTabs()))[0])?.conversationId) {
    assert.ok(Date.now() < deadline, 'Conversation identity timed out')
    await sleep(50)
  }
  const pending = 'PENDING_RESTART_SENTINEL', committed = 'COMMITTED_RESTART_SENTINEL'
  await page.evaluate(({ id, pending, committed }) => {
    window.kaminBridge.submitText(id, pending, 'pending-restart')
    window.kaminBridge.submitText(id, committed, 'committed-restart')
  }, { id: tab.id, pending, committed })
  await fixture.wait(frame => frame.type === 'session:submitText' && frame.submissionId === 'committed-restart')
  await context.close() // Receipt transitions persist while no browser exists.
  fixture.send(tab.conversationId, { type: 'session:submission', id: 'committed-restart', state: 'read', uuid: 'committed-uuid' })
  fixture.send(tab.conversationId, { type: 'jsonl:entries', entries: [
    { type: 'user', uuid: 'committed-uuid', _ord: 1, message: { role: 'user', content: committed } },
  ] })
  await fixture.tool('Write', { file_path: `${fixture.root}/restart-barrier.txt`, content: 'shared handler barrier' })
  // JSONL delivery to presentation is batched independently of tool responses.
  // Crash only after observing the durable commitment, with zero browser pages.
  const persistedDeadline = Date.now() + 15000
  for (;;) {
    const journal = JSON.parse(await fs.readFile(`${fixture.root}/storage/workspace-submissions.json`, 'utf8'))
    if (journal.submissions.some(item => item.receipt.id === 'committed-restart' && item.receipt.committed)) break
    assert.ok(Date.now() < persistedDeadline, 'Zero-page UUID commitment was not persisted')
    await sleep(50)
  }
  await fixture.restart(true) // SIGKILL our isolated host, never the bridge server.
  context = await browser.newContext()
  t.after(() => context.close())
  for (let n = 0; n < 2; n++) {
    const fresh = await open()
    page = fresh.page
    await page.getByText('Unconfirmed', { exact: true }).first().waitFor()
    const pendingReceipt = fresh.receipts.find(receipt => receipt.id === 'pending-restart')
    const committedReceipt = fresh.receipts.find(receipt => receipt.id === 'committed-restart')
    assert.equal(pendingReceipt?.state, 'uncertain')
    assert.equal(pendingReceipt?.cancelable, false)
    assert.match(pendingReceipt?.warning ?? '', /restarted/)
    assert.equal(committedReceipt?.committed, true)
    assert.equal(await page.locator('.jsonl-entry-user').filter({ hasText: pending }).count(), 1)
    const storedRows = await page.evaluate(id => (window.__jsonlEntries?.[id] ?? []).filter(row => row.uuid === 'committed-uuid').length, tab.id)
    assert.equal(await page.locator('.jsonl-entry-user').filter({ hasText: committed }).count(), storedRows,
      'A committed receipt must not become a new optimistic row after process restart')
    await page.evaluate(({ id, pending }) => window.kaminBridge.submitText(id, pending, 'pending-restart'), { id: tab.id, pending })
  }
  await page.evaluate(id => window.kaminBridge.submitText(id, 'new post-restart submission', 'new-restart'), tab.id)
  await fixture.wait(frame => frame.type === 'session:submitText' && frame.submissionId === 'new-restart')
  assert.equal(fixture.receivedFrames().filter(frame => frame.type === 'session:submitText' && frame.submissionId === 'pending-restart').length, 1)
  assert.equal(fixture.receivedFrames().filter(frame => frame.type === 'session:submitText' && frame.submissionId === 'committed-restart').length, 1)
})
