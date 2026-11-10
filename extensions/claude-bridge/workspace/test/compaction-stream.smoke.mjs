import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { WorkspaceAuth, startWorkspaceServer, createAppAssetHandler, withWorkspaceShell } from '../dist/index.js'

// The CLI's compaction request streams as a contentless `__compaction` marker
// (server/src/core/proxy/compaction-request.ts). Both the shipped IDE chat.html
// and the browser bundle must show "Compacting conversation…" like the CLI TUI,
// never an assistant bubble, and drop the indicator when the marker closes.
// Native host postMessage is emulated; this is not Windows CEF evidence.
test('a compaction stream shows the Compacting indicator and no answer in IDE chat and browser', { timeout: 60000 }, async t => {
  const tab = { id: 'compact-tab', status: 'connected', sessionTitle: 'Compaction fixture', cwd: '/workspace' }
  const rows = [
    { type: 'user', uuid: 'c-1', parentUuid: null, _ord: 1, timestamp: '2026-11-10T10:00:00Z',
      message: { role: 'user', content: 'Please compact this conversation.' } },
    { type: 'assistant', uuid: 'c-2', parentUuid: 'c-1', _ord: 2, timestamp: '2026-11-10T10:00:01Z',
      message: { id: 'answer-1', role: 'assistant', content: [{ type: 'text', text: 'Earlier answer stays visible.' }] } },
  ]
  const marker = streaming => ({ type: 'assistant', uuid: 'stream-compact-1', parentUuid: null, timestamp: '2026-11-10T10:00:02Z',
    isSidechain: false, __streaming: streaming, __compaction: true,
    message: { id: 'compact-1', role: 'assistant', model: 'claude-opus-5-5', content: [] } })
  const replies = {
    'get-version': 'fixture', 'get-config': { serverUrl: 'ws://fixture.invalid', token: '[fixture]', cwd: '/workspace' },
    'tab:list': [tab], 'tab:get-active': tab.id, 'tabs:restore-state': [], 'session:get-saved': [],
    'layout:get': {}, 'skills:list': [], 'vscode:is-available': false, 'mcp:status': [],
    'get-editor-selection': null, 'bridge:server-version': null, 'get-machine-id': 'compact-fixture',
    'hooks:list-pending-plugin-approvals': [], 'base-prompt:get': '', 'notifications:get': {}, 'terminal:get-prefs': {},
    'bridge:server-fetch': { ok: false, status: 404, data: null },
  }
  const replay = { kind: 'event', channel: 'jsonl-entries', args: [tab.id, rows] }
  const views = new Set()
  const assets = fileURLToPath(new URL('../../webview/dist/browser/', import.meta.url))
  const server = await startWorkspaceServer({ port: 0,
    auth: new WorkspaceAuth('compact-fixture-token', async () => ({ id: 'compact' })),
    backend: withWorkspaceShell({ attach(view) {
      views.add(view)
      return {
        async receive(frame) {
          if (frame.kind === 'invoke') await view.postMessage({ kind: 'invoke-reply', id: frame.id,
            generation: frame.generation, ok: true, result: replies[frame.channel] ?? null })
          if (frame.kind === 'send' && frame.channel === 'jsonl:request-replay') {
            setTimeout(() => { void view.postMessage(replay) }, 100)
          }
        }, dispose() { views.delete(view) },
      }
    } }), serveApp: await createAppAssetHandler(assets),
  })
  t.after(() => server.close())
  const nativeHtml = await readFile(new URL('../../../../builtin-extensions/claude-bridge/chat.html', import.meta.url), 'utf8')
  const native = createServer((req, res) => { res.setHeader('Content-Type', 'text/html'); res.end(nativeHtml) })
  await new Promise(resolve => native.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise(resolve => native.close(resolve)))
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, colorScheme: 'dark' })
  await context.route('**/*', route => ['127.0.0.1', 'localhost'].includes(new URL(route.request().url()).hostname)
    ? route.continue() : route.abort())

  const web = await context.newPage()
  await web.goto(server.origin)
  await web.locator('#workspace-token').fill('compact-fixture-token')
  await web.getByRole('button', { name: 'Sign in', exact: true }).click()
  const ide = await context.newPage()
  await ide.addInitScript(({ replies, replay }) => {
    let state
    window.acquireVsCodeApi = () => ({ getState: () => state, setState: next => { state = next },
      postMessage(frame) {
        if (frame.kind === 'invoke') queueMicrotask(() => window.postMessage({ kind: 'invoke-reply',
          id: frame.id, generation: frame.generation, ok: true, result: replies[frame.channel] ?? null }, '*'))
        if (frame.kind === 'send' && frame.channel === 'jsonl:request-replay') {
          setTimeout(() => window.postMessage(replay, '*'), 100)
        }
      } })
  }, { replies, replay })
  await ide.goto(`http://127.0.0.1:${native.address().port}`)

  const pages = [
    ['ide-chat-html', ide, frame => ide.evaluate(f => window.postMessage(f, '*'), frame)],
    ['browser-shell', web, async frame => { for (const view of views) await view.postMessage(frame) }],
  ]
  for (const [name, page, push] of pages) {
    await page.getByText('Earlier answer stays visible.').waitFor()
    const bubbles = await page.locator('.jsonl-entry-assistant').count()
    await push({ kind: 'event', channel: 'streaming-entry', args: [tab.id, marker(true)] })
    const indicator = page.getByRole('status').filter({ hasText: 'Compacting conversation…' })
    await indicator.waitFor()
    if (process.env.BRIDGE_BROWSER_SCREENSHOT) await page.screenshot({ path: path.join(path.dirname(process.env.BRIDGE_BROWSER_SCREENSHOT), `compaction-${name}.png`) })
    assert.equal(await page.locator('.jsonl-entry-assistant').count(), bubbles, `${name}: no assistant bubble for the summary`)
    await push({ kind: 'event', channel: 'streaming-entry', args: [tab.id, marker(false)] })
    await indicator.waitFor({ state: 'detached' })
    assert.equal(await page.locator('.jsonl-entry-assistant').count(), bubbles, `${name}: no ghost stub after the compaction`)
  }
})
