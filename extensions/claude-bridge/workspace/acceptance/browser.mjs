import { check, CheckFailure } from './report.mjs'

export async function raw(page, method, ...args) {
  return page.evaluate(({ method, args }) => window.bridgeCmd.raw(method, ...args), { method, args })
}
const notificationPages = new WeakSet()
async function handleNotifications(page) {
  if (notificationPages.has(page)) return
  // Host notifications use modal browser effects. Acknowledge only their real
  // Dismiss control; permission prompts and other browser actions stay intact.
  const messages = page.locator('dialog[aria-label="Bridge workspace action"]').filter({
    has: page.getByRole('button', { name: 'Dismiss', exact: true }),
  }).last()
  await page.addLocatorHandler(messages, async dialog => {
    await dialog.getByRole('button', { name: 'Dismiss', exact: true }).click()
  }, { noWaitAfter: true })
  notificationPages.add(page)
}
export async function ready(page, origin) {
  await handleNotifications(page)
  await page.goto(origin, { waitUntil: 'domcontentloaded' })
  await page.waitForFunction(() => !!window.bridgeCmd, null, { timeout: 30_000 })
}
export async function newTab(page, mode = 'bypassPermissions') {
  await handleNotifications(page)
  const config = await raw(page, 'getConfig')
  const id = await raw(page, 'createTab', { ...config, cwd: '/workspace' })
  await page.evaluate(id => window.bridgeCmd.switchTab(id), id)
  await raw(page, 'changePermissionMode', id, mode)
  return id
}
export async function turn(page, prompt) {
  await page.evaluate(prompt => window.bridgeCmd.sendAndWait(prompt, { timeoutMs: 180_000 }), prompt)
}
// Inspect the real viewer store, rather than truncated entry previews or model prose.
// Full content stays in browser memory; only IDs/counts/tool names cross to Node.
export async function transcript(page, nonce) {
  return page.evaluate(nonce => {
    const id = window.bridgeCmd.state().activeTabId
    const all = window.__jsonlEntries?.[id] ?? []
    const messages = all.filter(e => ['user', 'assistant'].includes(e.type))
    const blocks = e => typeof e.message?.content === 'string'
      ? [{ type: 'text', text: e.message.content }] : (e.message?.content ?? [])
    const text = e => blocks(e).filter(b => b.type === 'text').map(b => b.text ?? '').join('\n')
    const ids = messages.map(e => e.uuid).filter(Boolean)
    const calls = messages.flatMap(blocks).filter(b => b.type === 'tool_use')
    const results = messages.flatMap(blocks).filter(b => b.type === 'tool_result')
    return { ids, duplicateIds: ids.length - new Set(ids).size,
      users: messages.filter(e => e.type === 'user' && text(e).includes(nonce)).length,
      done: messages.some(e => e.type === 'assistant' && text(e).includes(`${nonce}:ok`)),
      tools: calls.map(b => b.name),
      unresolved: calls.filter(c => !results.some(r => r.tool_use_id === c.id)).length }
  }, nonce)
}
export function complete(snapshot, { baseline = [], code = 'TRANSCRIPT' } = {}) {
  check(snapshot.ids.length > 0 && snapshot.duplicateIds === 0 && snapshot.users === 1 && snapshot.done &&
    snapshot.unresolved === 0 && baseline.every(id => snapshot.ids.includes(id)), code)
}
export function used(snapshot, name) {
  check(snapshot.tools.some(n => n === name || n.endsWith(`__${name}`)), `TOOL_${name.toUpperCase()}`)
}
export async function heap(context, page) {
  const session = await context.newCDPSession(page)
  try {
    await session.send('HeapProfiler.collectGarbage')
    const { usedSize, totalSize } = await session.send('Runtime.getHeapUsage')
    return { usedBytes: usedSize, totalBytes: totalSize, method: 'CDP Runtime.getHeapUsage after GC' }
  } finally { await session.detach() }
}
export async function requirePanel(page, path, expected) {
  try { check((await raw(page, 'fileViewerRead', path)).content.includes(expected), 'FILE_PANEL') }
  catch { throw new CheckFailure('FILE_PANEL') }
  const show = page.locator('[data-tooltip="Show plan & files"]')
  if (await show.count()) {
    check(await show.count() === 1, 'FILE_PANEL_TOGGLE_COUNT')
    check(await show.isVisible(), 'FILE_PANEL_TOGGLE_VISIBLE')
    try { await show.click({ timeout: 10_000 }) }
    catch (error) {
      throw new CheckFailure(error.message.includes('intercepts pointer events') ? 'FILE_PANEL_TOGGLE_OBSCURED' : 'FILE_PANEL_TOGGLE_CLICK')
    }
  }
  // Select the real changed-file list; the panel initially shows its project tree.
  const activity = page.locator('.right-panel-column')
  try {
    await activity.waitFor({ state: 'visible', timeout: 10_000 })
  } catch { throw new CheckFailure('FILE_PANEL_ACTIVITY') }
  try { await activity.getByRole('button').filter({ has: page.getByText('Write', { exact: true }) }).click({ timeout: 10_000 }) }
  catch { throw new CheckFailure('FILE_PANEL_WRITE_TAB') }
  const file = activity.locator(`[data-tooltip="${path}"]`).first()
  try { await file.waitFor({ state: 'visible', timeout: 10_000 }); await file.click() }
  catch { throw new CheckFailure('FILE_PANEL_LIST') }
  try { await page.locator('.file-panel-column .cm-content').filter({ hasText: expected }).first().waitFor({ timeout: 10_000 }) }
  catch { throw new CheckFailure('FILE_PANEL_CONTENT') }
}
