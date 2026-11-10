import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { setTimeout as sleep } from 'node:timers/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { previewFixture } from './fixtures/preview-fixture.mjs'

test('browser restores named open sessions, selection and pins after real host restart', { timeout: 60000 }, async t => {
  const fixture = await previewFixture(t, { conversations: true })
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
  page.setDefaultTimeout(15000)
  // Playwright 1.65 treats a Promise predicate as truthy; poll awaited replies
  // in Node so a false disk read can never satisfy a restart precondition.
  async function waitRead(predicate, arg) {
    const deadline = Date.now() + 15000
    while (!await page.evaluate(predicate, arg)) {
      assert.ok(Date.now() < deadline, 'Host read condition timed out')
      await sleep(100)
    }
  }
  async function open() {
    await page.goto(fixture.origin)
    if (await page.locator('#workspace-token').isVisible()) {
      await page.locator('#workspace-token').fill('synthetic-preview-token')
      await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    }
    await page.getByRole('tablist', { name: 'Open sessions', exact: true }).waitFor()
  }
  await open()
  await page.getByRole('button', { name: 'No folder session', exact: true }).click()
  await waitRead(async () => !!(await window.kaminBridge.listTabs())[0]?.conversationId)
  const first = (await page.evaluate(() => window.kaminBridge.listTabs()))[0]
  fixture.send(first.conversationId, { type: 'session:title', title: 'pwd в файл' })
  const strip = page.getByRole('tablist', { name: 'Open sessions', exact: true })
  await strip.getByRole('tab', { name: /pwd в файл/ }).waitFor()
  await strip.getByRole('button', { name: 'Pin tab', exact: true }).press('Enter')
  await waitRead(async id => (await window.kaminBridge.restoreTabsState()).some(t => t.id === id && t.pinned), first.id)
  await page.getByRole('button', { name: 'New session', exact: true }).click()
  await waitRead(async () => { const tabs = await window.kaminBridge.listTabs(); return tabs.length === 2 && !!tabs[1].conversationId })
  await strip.locator(`[data-tab-id="${first.id}"]`).press('Enter')
  await waitRead(async id => { const tabs = await window.kaminBridge.restoreTabsState(); return tabs.some(t => t.id === id && t.pinned) && tabs.length === 2 }, first.id)
  await waitRead(async id => await window.kaminBridge.getActiveTab() === id, first.id)
  const before = await page.evaluate(() => window.kaminBridge.listTabs())
  assert.equal(before.length, 2)
  assert.equal((await page.evaluate(() => window.kaminBridge.restoreTabsState())).find(t => t.id === first.id)?.pinned, true)
  await fixture.restart()
  await open()
  await waitRead(async () => (await window.kaminBridge.listTabs()).length === 2)
  const restored = await page.evaluate(() => window.kaminBridge.listTabs())
  assert.deepEqual(restored.map(t => [t.id, t.conversationId, t.sessionTitle, t.label]), before.map(t => [t.id, t.conversationId, t.sessionTitle, t.label]))
  assert.equal(await page.evaluate(() => window.kaminBridge.getActiveTab()), first.id)
  await strip.locator(`[data-tab-id="${first.id}"]`).getByRole('button', { name: 'Unpin tab', exact: true }).waitFor()
  await strip.getByRole('tab', { name: /pwd в файл/ }).waitFor()
  assert.equal(await strip.locator('.sleeping').count(), 0)
  await page.reload()
  await strip.getByRole('tab', { name: /pwd в файл/ }).waitFor()
  assert.equal(await page.evaluate(() => window.kaminBridge.getActiveTab()), first.id)
  await strip.locator(`[data-tab-id="${first.id}"]`).getByRole('button', { name: 'Close tab', exact: true }).press('Space')
  await waitRead(async () => (await window.kaminBridge.listTabs()).length === 1)
  await fixture.wait(frame => frame.type === 'session:resume' && frame.conversationId === before[1].conversationId)
  await fixture.restart()
  await open()
  await waitRead(async () => (await window.kaminBridge.listTabs()).length === 1)
  assert.equal((await page.evaluate(() => window.kaminBridge.getSavedSessions())).find(s => s.conversationId === first.conversationId).label, 'pwd в файл')
  // Reopen via the actual history action, which omits sessionLabel in its IPC.
  await page.locator('.ws-project-header[aria-expanded="false"]').click()
  await page.locator('.ws-saved').filter({ hasText: 'pwd в файл' }).click()
  await strip.getByRole('tab', { name: /pwd в файл/ }).waitFor()
  assert.equal(await page.evaluate(() => localStorage.length), 0)
  assert.equal(await page.evaluate(() => sessionStorage.getItem('workspace-sessions')), null)
})
