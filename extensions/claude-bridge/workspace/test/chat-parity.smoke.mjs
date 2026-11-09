import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { WorkspaceAuth, startWorkspaceServer, createAppAssetHandler, withWorkspaceShell } from '../dist/index.js'

// One transcript, the shipped IDE chat.html and the production browser bundle.
// Native host palette/postMessage are emulated; this is not Windows CEF evidence.
test('IDE chat and browser share assistant surfaces, gutters and telemetry-spanning tool bursts', { timeout: 60000 }, async t => {
  const tab = { id: 'parity-tab', status: 'connected', sessionTitle: 'Parity fixture', cwd: '/workspace' }
  let previous = null
  let order = 0
  const entry = (type, fields = {}) => {
    const row = { type, uuid: `parity-${++order}`, parentUuid: previous, _ord: order,
      timestamp: '2026-11-09T20:00:00Z', ...fields }
    previous = row.uuid
    return row
  }
  const tool = name => entry('assistant', { message: { id: `message-${order}`, role: 'assistant',
    content: [{ type: 'tool_use', id: `tool-${name}`, name, input: {} }] } })
  const result = name => entry('user', { message: { role: 'user',
    content: [{ type: 'tool_result', tool_use_id: `tool-${name}`, content: 'ok' }] } })
  const rows = [entry('user', { message: { role: 'user', content: 'Read, write and verify the workspace.' } }),
    entry('assistant', { message: { id: 'opening-message', role: 'assistant',
      content: [{ type: 'text', text: 'I will inspect the workspace and verify the result.' }] } }),
    tool('ToolSearch'), result('ToolSearch'),
    ...['atis-latch', 'tools-used', 'api-request-shape', 'api-request-blob', 'api-request'].map(type => entry(type)),
    tool('Bash'), result('Bash'), entry('future-cli-telemetry'),
    tool('Read'), result('Read'),
    entry('assistant', { message: { id: 'final-message', role: 'assistant',
      content: [{ type: 'text', text: 'Verified the file in `/workspace`. All three tools completed.' }] } })]
  const replies = {
    'get-version': 'fixture', 'get-config': { serverUrl: 'ws://fixture.invalid', token: '[fixture]', cwd: '/workspace' },
    'tab:list': [tab], 'tab:get-active': tab.id, 'tabs:restore-state': [], 'session:get-saved': [],
    'layout:get': {}, 'skills:list': [], 'vscode:is-available': false, 'mcp:status': [],
    'get-editor-selection': null, 'bridge:server-version': null, 'get-machine-id': 'parity-fixture',
    'hooks:list-pending-plugin-approvals': [], 'base-prompt:get': '', 'notifications:get': {}, 'terminal:get-prefs': {},
    'bridge:server-fetch': { ok: false, status: 404, data: null },
  }
  const replay = { kind: 'event', channel: 'jsonl-entries', args: [tab.id, rows] }
  const assets = fileURLToPath(new URL('../../webview/dist/browser/', import.meta.url))
  const server = await startWorkspaceServer({ port: 0,
    auth: new WorkspaceAuth('parity-fixture-token', async () => ({ id: 'parity' })),
    backend: withWorkspaceShell({ attach(view) { return {
      async receive(frame) {
        if (frame.kind === 'invoke') await view.postMessage({ kind: 'invoke-reply', id: frame.id,
          generation: frame.generation, ok: true, result: replies[frame.channel] ?? null })
        if (frame.kind === 'send' && frame.channel === 'jsonl:request-replay') {
          setTimeout(() => { void view.postMessage(replay) }, 100)
        }
      }, dispose() {},
    } } }), serveApp: await createAppAssetHandler(assets),
  })
  t.after(() => server.close())
  const nativeHtml = await readFile(new URL('../../../../builtin-extensions/claude-bridge/chat.html', import.meta.url), 'utf8')
  const native = createServer((req, res) => { res.setHeader('Content-Type', 'text/html'); res.end(nativeHtml) })
  await new Promise(resolve => native.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise(resolve => native.close(resolve)))
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext({ viewport: { width: 1600, height: 900 }, colorScheme: 'dark' })
  await context.route('**/*', route => ['127.0.0.1', 'localhost'].includes(new URL(route.request().url()).hostname)
    ? route.continue() : route.abort())
  const web = await context.newPage()
  await web.goto(server.origin)
  await web.locator('#workspace-token').fill('parity-fixture-token')
  await web.getByRole('button', { name: 'Sign in', exact: true }).click()
  const webPanel = web.locator('.ws-panel-content').filter({ has: web.locator('#content-area') })
  await webPanel.waitFor()
  const panelSize = await webPanel.evaluate(element => ({ width: element.clientWidth, height: element.clientHeight }))
  const ide = await context.newPage()
  await ide.setViewportSize(panelSize)
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
    document.addEventListener('DOMContentLoaded', () => document.body.classList.add('vscode-dark'))
  }, { replies, replay })
  await ide.goto(`http://127.0.0.1:${native.address().port}`)
  // GPUI injects these theme keys into the document; browser-theme emulates them.
  await ide.addStyleTag({ content: `:root { --vscode-editor-background:#313240; --vscode-editorWidget-background:#262533;
    --vscode-input-background:#3d3f51; --vscode-font-size:13px; --vscode-foreground:#cfd4e2;
    --vscode-descriptionForeground:#838aa0; --vscode-disabledForeground:#60667b; --vscode-focusBorder:#89b4fa;
    --vscode-list-hoverBackground:rgba(61,63,81,0.5); }
    html,body { background-color:var(--vscode-editor-background); color:var(--vscode-foreground); }` })
  const measurements = []
  for (const [name, page] of [['ide-chat-html', ide], ['browser-shell', web]]) {
    await page.waitForFunction(() => document.querySelectorAll('.tool-group-title').length === 1 &&
      document.querySelector('.tool-group-title').textContent.includes('3 tool calls'))
    await page.getByText('All three tools completed.', { exact: false }).waitFor()
    assert.deepEqual(await page.locator('.tool-group-chip').allTextContents(), ['ToolSearch ×1', 'Bash ×1', 'Read ×1'])
    const metrics = await page.evaluate(() => {
      const css = selector => {
        const element = document.querySelector(selector), style = getComputedStyle(element)
        return { background: style.backgroundColor, padding: style.padding, margin: style.margin }
      }
      const assistant = document.querySelector('.asst-merge-container')
      const identity = assistant.querySelector('.entry-role-label')
      let surface = document.querySelector('#content-area').parentElement
      while (surface && getComputedStyle(surface).backgroundColor === 'rgba(0, 0, 0, 0)') surface = surface.parentElement
      return { assistant: css('.asst-merge-container'), header: css('.jsonl-entry-assistant .entry-header'),
        composer: css('.input-bar'), user: css('.jsonl-entry-user'), tool: css('.tool-group-header'),
        gutter: identity.getBoundingClientRect().left - assistant.getBoundingClientRect().left,
        panel: getComputedStyle(document.querySelector('#content-area').parentElement).backgroundColor,
        effectivePanel: getComputedStyle(surface).backgroundColor }
    })
    assert.equal(metrics.assistant.background, 'rgba(0, 0, 0, 0)')
    assert.equal(metrics.composer.background, 'rgba(0, 0, 0, 0)')
    assert.equal(metrics.panel, 'rgb(49, 50, 64)')
    assert.equal(metrics.gutter, 14)
    measurements.push({ name, metrics })
    // Expansion must still expose each original call and result.
    await page.locator('.tool-group-header').click()
    await page.locator('.tool-group-header[aria-expanded="true"]').waitFor()
    assert.deepEqual(await page.locator('.tool-group-body .tool-name').allTextContents(), ['ToolSearch', 'Bash', 'Read'])
    assert.equal(await page.locator('.tool-group-body .tool-result-inline').count(), 3)
    await page.locator('.tool-group-header').first().click()
    await page.mouse.move(0, 0)
    if (process.env.BRIDGEWEB_ACC_CONTAINER) {
      const panel = name === 'browser-shell' ? webPanel : page.locator('#root')
      await panel.screenshot({ path: `/reports/${name}.png` })
    }
  }
  assert.deepEqual(measurements[1].metrics, measurements[0].metrics)
  if (process.env.BRIDGEWEB_ACC_CONTAINER) await writeFile('/reports/chat-computed-styles.json', JSON.stringify(measurements, null, 2))
})
