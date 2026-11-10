// Production composer, реальный shared host и диск; upstream синтетический.
import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { previewFixture } from './fixtures/preview-fixture.mjs'

test('composer drafts survive zero pages and host restart, isolate sessions and resolve real page conflicts', { timeout: 90000 }, async t => {
  const fixture = await previewFixture(t, { conversations: true })
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } })
  await context.addInitScript(() => {
    // Delay one actual UI-generated update to reproduce a stale competing page.
    const Socket = window.WebSocket
    window.holdDrafts = false
    window.releaseDrafts = () => {}
    window.WebSocket = class extends Socket {
      held = []
      constructor(...args) { super(...args); window.releaseDrafts = () => { for (const data of this.held.splice(0)) super.send(data) } }
      send(data) {
        const frame = JSON.parse(data)
        if (window.holdDrafts && frame.kind === 'invoke' && frame.channel === 'workspace:draft:update') this.held.push(data)
        else super.send(data)
      }
    }
  })
  const errors = []
  async function open() {
    const page = await context.newPage()
    page.setDefaultTimeout(15000)
    page.on('pageerror', error => {
      if (error.message !== 'Unsupported workspace invoke: get-editor-selection') errors.push(error.message)
    })
    await page.goto(fixture.origin)
    if (await page.locator('#workspace-token').isVisible()) {
      await page.locator('#workspace-token').fill('synthetic-preview-token')
      await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    }
    await page.getByRole('tablist', { name: 'Open sessions', exact: true }).waitFor()
    return page
  }
  async function waitRead(page, predicate, arg) {
    const deadline = Date.now() + 15000
    while (!await page.evaluate(predicate, arg)) {
      assert.ok(Date.now() < deadline, 'Host draft condition timed out')
      await sleep(50)
    }
  }
  async function saved(page, text) {
    await page.waitForFunction(expected => document.querySelector('#prompt-input')?.value === expected, text)
    await page.locator('[data-workspace-draft="saved"]').waitFor()
    await waitRead(page, async expected => {
      const id = await window.kaminBridge.getActiveTab()
      return (await window.kaminBridge.invoke('workspace:draft:get', id)).value.text === expected
    }, text)
  }
  let a = await open()
  await a.getByRole('button', { name: 'No folder session', exact: true }).click()
  await waitRead(a, async () => !!(await window.kaminBridge.listTabs())[0]?.conversationId)
  const first = (await a.evaluate(() => window.kaminBridge.listTabs()))[0]
  await a.locator('[data-workspace-draft="saved"]').waitFor()
  await a.locator('#prompt-input').fill('Unsent durable composer draft')
  await saved(a, 'Unsent durable composer draft')
  const b = await open()
  await saved(b, 'Unsent durable composer draft')
  await a.locator('.attach-btn').click()
  await a.getByLabel('Attachments', { exact: true }).setInputFiles({ name: 'draft-proof.txt', mimeType: 'text/plain', buffer: Buffer.from('persistent attachment bytes') })
  await a.getByRole('button', { name: 'Upload', exact: true }).click()
  await a.locator('.attachment-item').filter({ hasText: 'draft-proof.txt' }).waitFor()
  await b.locator('.attachment-item').filter({ hasText: 'draft-proof.txt' }).waitFor()
  await waitRead(a, async id => (await window.kaminBridge.invoke('workspace:draft:get', id)).value.attachments.length === 1, first.id)
  const attachment = await a.evaluate(async id => (await window.kaminBridge.invoke('workspace:draft:get', id)).value.attachments[0], first.id)
  assert.equal(await fs.readFile(attachment.path, 'utf8'), 'persistent attachment bytes')
  assert.ok(attachment.path.startsWith(fixture.root + path.sep))
  for (const choice of ['Keep this draft', 'Use workspace draft']) {
    await a.evaluate(() => { window.holdDrafts = true })
    await a.locator('#prompt-input').fill('Local competing edit ' + choice)
    await b.locator('#prompt-input').fill('Other page ' + choice)
    await saved(b, 'Other page ' + choice)
    await a.locator('[data-workspace-draft="conflict"]').waitFor()
    assert.equal(await a.locator('#prompt-input').inputValue(), 'Local competing edit ' + choice)
    await a.evaluate(() => { window.holdDrafts = false; window.releaseDrafts() })
    await waitRead(a, async () => { await window.kaminBridge.invoke('get-version'); return true })
    await a.getByRole('button', { name: choice, exact: true }).click()
    const text = (choice === 'Keep this draft' ? 'Local competing edit ' : 'Other page ') + choice
    await saved(a, text); await saved(b, text)
  }
  const retained = 'Other page Use workspace draft'
  await a.getByRole('button', { name: 'New session', exact: true }).click()
  await waitRead(a, async () => (await window.kaminBridge.listTabs()).length === 2)
  await saved(a, '')
  await a.locator('#prompt-input').fill('Second session draft')
  await saved(a, 'Second session draft')
  await a.getByRole('tablist', { name: 'Open sessions', exact: true }).locator(`[data-tab-id="${first.id}"]`).press('Enter')
  await saved(a, retained)
  assert.equal(await a.locator('.attachment-item').filter({ hasText: 'draft-proof.txt' }).count(), 1)
  await Promise.all([a.close(), b.close()])
  t.diagnostic('restarting host with acknowledged draft')
  await fixture.restart()
  a = await open()
  await saved(a, retained)
  await a.locator('.attachment-item').filter({ hasText: 'draft-proof.txt' }).waitFor()
  assert.equal(await a.evaluate(() => localStorage.length), 0)
  assert.equal(await a.evaluate(() => Object.keys(sessionStorage).some(key => /draft|attachment/.test(key))), false)
  await waitRead(a, async id => (await window.kaminBridge.listTabs()).find(t => t.id === id)?.status === 'connected', first.id)
  // The synthetic upstream must finish the resumed session replay, as the real
  // server does; the composer correctly waits for that completion before Send.
  fixture.send(first.conversationId, { type: 'jsonl:status', status: 'watching', replayComplete: true })
  t.diagnostic('restored text and attachment; submitting through actual composer')
  const before = fixture.receivedFrames().filter(f => f.type === 'session:submitText').length
  await a.waitForFunction(() => !document.querySelector('#input-bar')?.classList.contains('disabled'))
  await a.locator('#prompt-input').press('Enter')
  const sent = await fixture.wait(f => f.type === 'session:submitText' && f.data.includes(retained))
  assert.ok(sent.data.includes(attachment.path))
  await saved(a, '')
  await a.close()
  await fixture.restart()
  a = await open()
  await saved(a, '')
  assert.equal(await a.locator('.attachment-item').count(), 0)
  assert.equal(fixture.receivedFrames().filter(f => f.type === 'session:submitText').length, before + 1)
  assert.deepEqual(errors, [])
})
