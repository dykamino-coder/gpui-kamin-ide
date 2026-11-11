import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { WorkspaceAuth, startWorkspaceServer, createAppAssetHandler, withWorkspaceShell } from '../dist/index.js'

// Synthetic prose only. Both production bundles run in Linux Chromium; the
// IDE postMessage boundary is emulated, not a Windows CEF acceptance claim.
test('long assistant prose stays complete and readable across streaming and canonical replay', { timeout: 60000 }, async t => {
  const tab = { id: 'scroll-tab', status: 'connected', sessionTitle: 'Scroll fixture', cwd: '/workspace' }
  const user = { type: 'user', uuid: 'scroll-user', _ord: 1,
    message: { role: 'user', content: 'Generate a detailed verification report.' } }
  const markers = ['OPENING_SENTINEL', 'PROJECT_SENTINEL', 'SERVER_SENTINEL']
  const blocks = [{ type: 'thinking', thinking: 'Inspect then verify.' },
    ...markers.map((marker, section) => ({ type: 'text', text: `${marker}\n\n` +
      Array.from({ length: 12 }, (_, n) => `- Section ${section}, check ${n}: ${'Verified the persistent workspace. '.repeat(3)}`).join('\n') }))]
  const canonical = blocks.map((block, i) => ({ type: 'assistant', uuid: `scroll-canonical-${i}`,
    parentUuid: i ? `scroll-canonical-${i - 1}` : user.uuid, _ord: i + 2,
    message: { id: 'scroll-message', role: 'assistant', content: [block] } }))
  const replay = { kind: 'event', channel: 'jsonl-entries', args: [tab.id, [user]] }
  const replies = {
    'get-version': 'fixture', 'get-config': { serverUrl: 'ws://fixture.invalid', token: '[fixture]', cwd: '/workspace' },
    'tab:list': [tab], 'tab:get-active': tab.id, 'tabs:restore-state': [], 'session:get-saved': [],
    'layout:get': {}, 'skills:list': [], 'vscode:is-available': false, 'mcp:status': [],
    'get-editor-selection': null, 'bridge:server-version': null, 'get-machine-id': 'scroll-fixture',
    'hooks:list-pending-plugin-approvals': [], 'base-prompt:get': '', 'notifications:get': {}, 'terminal:get-prefs': {},
  }
  let webView
  const server = await startWorkspaceServer({ port: 0,
    auth: new WorkspaceAuth('scroll-fixture-token', async () => ({ id: 'scroll' })),
    backend: withWorkspaceShell({ attach(view) { webView = view; return {
      async receive(frame) {
        if (frame.kind === 'invoke') await view.postMessage({ kind: 'invoke-reply', id: frame.id,
          generation: frame.generation, ok: true, result: replies[frame.channel] ?? null })
        if (frame.kind === 'send' && frame.channel === 'jsonl:request-replay') {
          setTimeout(() => { void view.postMessage(replay) }, 100)
        }
      }, dispose() {},
    } } }), serveApp: await createAppAssetHandler(fileURLToPath(new URL('../../webview/dist/browser/', import.meta.url))),
  })
  t.after(() => server.close())
  const html = await readFile(new URL('../../../../builtin-extensions/claude-bridge/chat.html', import.meta.url), 'utf8')
  const native = createServer((req, res) => { res.setHeader('Content-Type', 'text/html'); res.end(html) })
  await new Promise(resolve => native.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise(resolve => native.close(resolve)))
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  for (const name of ['ide-chat-html', 'browser-shell']) await t.test(name, async () => {
    const context = await browser.newContext({ viewport: { width: 1100, height: 800 } })
    try {
      await context.route('**/*', route => ['127.0.0.1', 'localhost'].includes(new URL(route.request().url()).hostname)
        ? route.continue() : route.abort())
      const page = await context.newPage()
      if (name === 'ide-chat-html') {
        await page.addInitScript(({ replies, replay }) => {
          window.acquireVsCodeApi = () => ({ getState: () => null, setState() {}, postMessage(frame) {
            if (frame.kind === 'invoke') queueMicrotask(() => window.postMessage({ kind: 'invoke-reply',
              id: frame.id, generation: frame.generation, ok: true, result: replies[frame.channel] ?? null }, '*'))
            if (frame.kind === 'send' && frame.channel === 'jsonl:request-replay') {
              setTimeout(() => window.postMessage(replay, '*'), 100)
            }
          } })
        }, { replies, replay })
        await page.goto(`http://127.0.0.1:${native.address().port}`)
      } else {
        await page.goto(server.origin)
        await page.locator('#workspace-token').fill('scroll-fixture-token')
        await page.getByRole('button', { name: 'Sign in', exact: true }).click()
      }
      await page.waitForFunction(() => window.__jsonlEntries?.['scroll-tab']?.length > 0)
      const emit = async (channel, value) => {
        const frame = { kind: 'event', channel, args: [tab.id, value] }
        if (name === 'browser-shell') await webView.postMessage(frame)
        else await page.evaluate(frame => window.postMessage(frame, '*'), frame)
      }
      const snapshot = content => ({ type: 'assistant', uuid: 'stream-scroll', __streaming: true,
        __streamingActiveBlock: content.length - 1, message: { id: 'scroll-message', role: 'assistant', content } })
      for (let i = 1; i < blocks.length; i++) {
        await emit('streaming-entry', snapshot([...blocks.slice(0, i), { type: 'text', text: '' }]))
        for (let offset = 0; offset < blocks[i].text.length; offset += 200) {
          await emit('streaming-delta', { msgId: 'scroll-message', blockIdx: i, kind: 'text', appendText: blocks[i].text.slice(offset, offset + 200) })
        }
        await page.waitForFunction(marker => document.querySelector('#content-area').innerText.includes(marker), markers[i - 1])
      }
      await emit('streaming-entry', { ...snapshot(blocks), __streaming: false })
      for (const row of canonical) await emit('jsonl-entries', [row])
      // A delayed proxy flush must not replace canonical prose with a partial
      // snapshot. Replaying the same UUIDs afterwards cannot repair that loss.
      await emit('streaming-entry', snapshot([blocks.at(-1)]))
      await emit('streaming-delta', { msgId: 'scroll-message', blockIdx: 0, kind: 'text', appendText: ' LATE_PROXY_SENTINEL' })
      // Duplicate replay must neither drop nor double any canonical block.
      await emit('jsonl-entries', canonical)
      await page.waitForFunction(expected => {
        const entry = window.__jsonlEntries?.['scroll-tab']?.find(e => e.message?.id === 'scroll-message')
        return entry?.__streaming === undefined && JSON.stringify(entry.message.content) === JSON.stringify(expected)
      }, blocks)
      // Thinking is readable text too, marked by a lightbulb; the answer is the direct children.
      const prose = page.locator('.jsonl-entry-assistant > .block-text')
      const thinking = page.locator('.jsonl-entry-assistant .block-thinking')
      assert.equal(await thinking.locator('.block-text').innerText(), blocks[0].thinking)
      assert.equal(await thinking.locator('.fa-lightbulb').count(), 1)
      assert.equal((await thinking.innerText()).trim(), blocks[0].thinking, 'no label, no collapsed teaser')
      await page.waitForFunction(markers => markers.every(marker => document.querySelector('#content-area').innerText.includes(marker)), markers)
      const texts = await prose.allTextContents()
      assert.equal(texts.length, 3)
      for (const marker of markers) assert.equal(texts.join('\n').split(marker).length - 1, 1)
      for (let i = 0; i < texts.length; i++) {
        for (const line of blocks[i + 1].text.split('\n').filter(Boolean)) {
          assert(texts[i].includes(line.replace(/^- /, '')), 'every prose line reaches the DOM')
        }
      }
      // A characterData mutation while already at bottom requests two no-op
      // snaps. Browsers emit no scroll event for writing an unchanged offset.
      await page.waitForTimeout(250)
      await page.evaluate(() => window.__scrollChatToBottom())
      const mutate = () => page.evaluate(() => {
        const text = document.querySelector('.jsonl-entry-assistant > .block-text').firstChild
        text.textContent += ' '
      })
      await mutate()
      await page.waitForTimeout(150)
      const metrics = () => page.locator('.chat-panel').evaluate(el => ({ top: el.scrollTop, max: el.scrollHeight - el.clientHeight }))
      const bottom = await metrics()
      assert(bottom.max > 450, 'long answer exceeds the chat viewport')
      await page.locator('.chat-panel').hover()
      await page.mouse.wheel(0, -450)
      await page.waitForTimeout(150)
      const reading = await metrics()
      assert(reading.top < bottom.top - 200, 'first wheel reaches earlier prose')
      await mutate()
      await page.waitForTimeout(150)
      const after = await metrics()
      t.diagnostic(JSON.stringify({ host: name, bottom, reading, after }))
      assert(Math.abs(after.top - reading.top) < 2, `reader must stay at ${reading.top}, reached ${after.top}`)
      // The opening must be accessible, not just present outside the viewport.
      await page.mouse.wheel(0, -10000)
      await page.waitForTimeout(150)
      const opening = await prose.first().locator('p').first().boundingBox()
      const panel = await page.locator('.chat-panel').boundingBox()
      assert(opening.y >= panel.y && opening.y + opening.height <= panel.y + panel.height,
        `opening paragraph ${JSON.stringify(opening)} must be inside chat ${JSON.stringify(panel)}`)
    } finally { await context.close() }
  })
})
