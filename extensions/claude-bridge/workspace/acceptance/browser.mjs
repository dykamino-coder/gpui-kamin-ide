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
async function turnWaitFailure(page, code) {
  try {
    const state = await page.evaluate(() => {
      const s = window.bridgeCmd.state()
      return { stateEntries: s.entryCount, busy: s.busy, working: s.working,
        promptReady: s.promptReady, settling: s.settling, queued: s.queued,
        boundMatchesActive: s.boundTabId === s.activeTabId }
    })
    console.log('turn-state:', JSON.stringify({ stage: code, ...state }))
  } catch { /* A lost page must not replace the original fixed failure code. */ }
  throw new CheckFailure(code)
}
export async function turn(page, prompt) {
  const marker = prompt.match(/acceptance-[a-f0-9-]+/)?.[0]
  check(marker, 'TURN_MARKER')
  const tab = await page.evaluate(prompt => window.bridgeCmd.send(prompt), prompt)
  // An old prompt-ready frame can briefly precede the submitted turn. Require
  // its own user entry before measuring quiescence, rather than pre-send calm.
  await page.waitForFunction(({ tab, marker }) => {
    const state = window.bridgeCmd.state()
    const entries = window.__jsonlEntries?.[tab] ?? []
    return state.activeTabId === tab && entries.some(e => e.type === 'user' &&
      (typeof e.message?.content === 'string' ? e.message.content :
        (e.message?.content ?? []).filter(b => b.type === 'text').map(b => b.text ?? '').join('\n')).includes(marker))
  }, { tab, marker }, { timeout: 180_000 }).catch(() => turnWaitFailure(page, 'TURN_ENTRY_WAIT'))
  await page.evaluate(() => window.bridgeCmd.waitForIdle({ timeoutMs: 180_000 }))
    .catch(() => turnWaitFailure(page, 'TURN_IDLE_WAIT'))
}
// Inspect the real viewer store, rather than truncated entry previews or model prose.
// Full content stays in browser memory; only IDs/counts/tool names cross to Node.
export async function transcript(page, nonce) {
  return page.evaluate(nonce => {
    const state = window.bridgeCmd.state()
    const id = state.activeTabId
    const all = window.__jsonlEntries?.[id] ?? []
    const messages = all.filter(e => ['user', 'assistant'].includes(e.type))
    const blocks = e => typeof e.message?.content === 'string'
      ? [{ type: 'text', text: e.message.content }] : (e.message?.content ?? [])
    const text = e => blocks(e).filter(b => b.type === 'text').map(b => b.text ?? '').join('\n')
    const ids = messages.map(e => e.uuid).filter(Boolean)
    const calls = messages.flatMap(blocks).filter(b => b.type === 'tool_use')
    const results = messages.flatMap(blocks).filter(b => b.type === 'tool_result')
    return { stateEntries: state.entryCount, renderedEntries: all.length, busy: state.busy,
      working: state.working, promptReady: state.promptReady, settling: state.settling,
      boundMatchesActive: state.boundTabId === id, ids, duplicateIds: ids.length - new Set(ids).size,
      users: messages.filter(e => e.type === 'user' && text(e).includes(nonce)).length,
      done: messages.some(e => e.type === 'assistant' && text(e).includes(`${nonce}:ok`)),
      tools: calls.map(b => b.name),
      unresolved: calls.filter(c => !results.some(r => r.tool_use_id === c.id)).length }
  }, nonce)
}
export function complete(snapshot, { baseline = [], code = 'TRANSCRIPT' } = {}) {
  if (!snapshot.ids.length) console.log('transcript-state:', JSON.stringify({
    stateEntries: snapshot.stateEntries, renderedEntries: snapshot.renderedEntries, busy: snapshot.busy,
    working: snapshot.working, promptReady: snapshot.promptReady, settling: snapshot.settling,
    boundMatchesActive: snapshot.boundMatchesActive }))
  const assertions = [
    [snapshot.ids.length > 0, 'EMPTY'], [snapshot.duplicateIds === 0, 'DUPLICATE'],
    [snapshot.users === 1, 'USER_COUNT'], [snapshot.done, 'REPLY'],
    [snapshot.unresolved === 0, 'UNRESOLVED_TOOL'], [baseline.every(id => snapshot.ids.includes(id)), 'BASELINE'],
  ]
  for (const [valid, reason] of assertions) check(valid, code === 'TRANSCRIPT' ? `${code}_${reason}` : code)
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
