import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { setTimeout as sleep } from 'node:timers/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { previewFixture } from './fixtures/preview-fixture.mjs'

// Synthetic upstream; real shared host, bounded replay, authenticated transport
// and production browser bundle. No model or operator workspace is involved.
test('native submissions committed with zero pages stay absent from optimistic rows in two fresh pages', { timeout: 60000 }, async t => {
  const fixture = await previewFixture(t, { conversations: true })
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext()
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
  let page = await open()
  await page.getByRole('button', { name: 'No folder session', exact: true }).click()
  let tab
  const deadline = Date.now() + 15000
  while (!(tab = (await page.evaluate(() => window.kaminBridge.listTabs()))[0])?.conversationId) {
    assert.ok(Date.now() < deadline, 'Conversation identity timed out')
    await sleep(50)
  }
  const text = 'COMMITTED_NATIVE_SENTINEL'
  await page.evaluate(({ id, text }) => window.kaminBridge.submitText(id, text, 'native-identity'), { id: tab.id, text })
  await fixture.wait(frame => frame.type === 'session:submitText' && frame.submissionId === 'native-identity')
  await context.close() // No browser sockets remain when the receipt and row land.
  fixture.send(tab.conversationId, { type: 'session:submission', id: 'native-identity', state: 'read', uuid: 'stored-native' })
  fixture.send(tab.conversationId, { type: 'jsonl:entries', entries: [
    { type: 'user', uuid: 'stored-native', _ord: 1, message: { role: 'user', content: text } },
  ] })
  // Push the accepted row beyond the real host's 4500-record replay tail.
  fixture.send(tab.conversationId, { type: 'jsonl:entries', entries: Array.from({ length: 4600 }, (_, n) =>
    ({ type: 'system', uuid: `later-${n}`, _ord: n + 2, content: '' })) })
  fixture.send(tab.conversationId, { type: 'jsonl:status', status: 'watching', replayComplete: true })
  await fixture.tool('Write', { file_path: `${fixture.root}/ordered.txt`, content: 'upstream barrier' })
  const fresh = await browser.newContext()
  t.after(() => fresh.close())
  for (let n = 0; n < 2; n++) {
    page = await fresh.newPage()
    page.setDefaultTimeout(15000)
    const receipts = []
    const batches = []
    let snapshotComplete = false
    page.on('websocket', socket => socket.on('framereceived', event => {
      const frame = JSON.parse(event.payload.toString())
      if (frame.channel === 'session:submission') receipts.push(frame.args[1])
      if (frame.channel === 'jsonl-entries') batches.push(frame.args[1].length)
      if (frame.channel === 'workspace:snapshot-end') snapshotComplete = true
    }))
    await page.goto(fixture.origin)
    if (await page.locator('#workspace-token').isVisible()) {
      await page.locator('#workspace-token').fill('synthetic-preview-token')
      await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    }
    try {
      await page.waitForFunction(id => window.__jsonlEntries?.[id]?.some(row => row._ord === 4601), tab.id)
    } catch (error) {
      t.diagnostic(JSON.stringify({ batches, receipts, store: await page.evaluate(() =>
        Object.entries(window.__jsonlEntries ?? {}).map(([id, rows]) => ({ id, count: rows.length, last: rows.at(-1)?._ord }))) }))
      throw error
    }
    // Transcript chunks precede receipts in the snapshot. Rendering its final
    // row does not prove the browser has received the remaining WS frames.
    const snapshotDeadline = Date.now() + 15000
    while (!snapshotComplete) {
      assert.ok(Date.now() < snapshotDeadline, 'Host snapshot did not complete')
      await sleep(25)
    }
    const receipt = receipts.find(receipt => receipt.id === 'native-identity')
    assert.equal(receipt?.committed, true, 'Host snapshot must carry commitment to each new page')
    assert.equal(await page.evaluate(id => window.__jsonlEntries[id].some(row => row.uuid === 'stored-native'), tab.id), false)
    assert.equal(await page.locator('.jsonl-entry-user').filter({ hasText: text }).count(), 0,
      'A trimmed committed submission must not return as a new optimistic message')
  }
})
