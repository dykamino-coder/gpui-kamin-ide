// Production browser + real shared host; synthetic upstream/MCP, no model.
import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { mcpBrowserEvidence } from '../acceptance/mcp-browser-evidence.mjs'
import { browserMcp } from './fixtures/browser-mcp.mjs'
import { previewFixture } from './fixtures/preview-fixture.mjs'

test('shared browser MCP APIs, two-page events and zero-page external execution', { timeout: 90000 }, async t => {
  const fixture = await previewFixture(t, { conversations: true })
  await writeFile(path.join(fixture.root, 'mcp-resource.txt'), 'independently seeded MCP resource')
  const first = await browserMcp(t, fixture.root, 'first')
  const second = await browserMcp(t, fixture.root, 'second')
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const context = await browser.newContext()
  await context.route('**/*', route => new URL(route.request().url()).origin === fixture.origin
    ? route.continue() : route.abort())
  async function open() {
    const page = await context.newPage()
    await page.goto(fixture.origin)
    if (await page.locator('#workspace-token').isVisible()) {
      await page.locator('#workspace-token').fill('synthetic-preview-token')
      await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    }
    await page.getByRole('tablist', { name: 'Open sessions', exact: true }).waitFor()
    await page.evaluate(() => {
      window.__mcpProof = { servers: [], logs: [], cleared: [], progress: [] }
      window.kaminBridge.onMcpServersChanged(value => window.__mcpProof.servers.push(value))
      window.kaminBridge.onMcpTestLog(value => window.__mcpProof.logs.push(value))
      window.kaminBridge.onMcpTestLogCleared(value => window.__mcpProof.cleared.push(value))
      window.kaminBridge.onMcpProgress(value => window.__mcpProof.progress.push(value))
    })
    return page
  }
  let a = await open(), b = await open()
  async function api(method, ...args) {
    return a.evaluate(({ method, args }) => window.kaminBridge[method](...args), { method, args })
  }
  async function eventually(assertions) {
    const deadline = Date.now() + 15000
    for (;;) {
      try { return await assertions() } catch (error) {
        if (Date.now() >= deadline) throw error
        await sleep(50)
      }
    }
  }
  const persisted = async () => JSON.parse(await readFile(path.join(fixture.root, 'home', '.claude.json'), 'utf8')).mcpServers
  const proof = mcpBrowserEvidence()
  await proof.prove('status', async () => assert.equal(await api('getMcpStatus'), true))
  await proof.prove('list', async () => assert.deepEqual(await api('getMcpServers'), []))
  let server
  await proof.prove('add', async () => {
    server = await api('addMcpServer', { name: 'browser_fixture', type: 'http', url: first.url, enabled: true })
    assert.equal(typeof server.id, 'string')
    await eventually(async () => {
      const live = (await api('getMcpServers')).find(item => item.id === server.id)
      assert.equal(live?.status, 'connected')
      assert.deepEqual(live.tools, ['fixture_record', 'fixture_second'])
      assert.equal(live.resources.length, 2)
      assert.deepEqual(live.resourceTemplates, first.templates)
      assert.equal(live.prompts.length, 2)
      for (const page of [a, b]) assert.ok((await page.evaluate(() => window.__mcpProof.servers))
        .some(servers => servers.some(item => item.id === server.id && item.status === 'connected' && item.prompts.length === 2)))
      assert.equal((await persisted()).browser_fixture.url, first.url)
    })
  })
  await proof.prove('catalog', async () => {
    assert.deepEqual(await api('getMcpResources', server.id), first.resources)
    assert.deepEqual(await api('getMcpPrompts', server.id), first.prompts)
    for (const method of ['tools/list', 'resources/list', 'resources/templates/list', 'prompts/list']) {
      assert.deepEqual(first.requests.filter(request => request.method === method).map(request => request.params?.cursor), [undefined, 'second'])
    }
  })
  await proof.prove('resource', async () => {
    const uri = first.resources[1].uri
    assert.deepEqual(await api('readMcpResource', server.id, uri), {
      ok: true, result: { contents: [{ uri, mimeType: 'text/plain', text: 'independently seeded MCP resource' }] },
    })
    assert.equal((await api('readMcpResource', 'missing-server', uri)).ok, false)
    assert.equal(first.requests.filter(request => request.method === 'resources/read').length, 1)
  })
  await proof.prove('prompt', async () => {
    assert.deepEqual(await api('getMcpPrompt', server.id, 'prompt_2', { topic: 'browser argument' }), {
      ok: true, result: { messages: [{ role: 'user', content: { type: 'text', text: 'first:browser argument' } }] },
    })
    assert.equal((await api('getMcpPrompt', 'missing-server', 'prompt_2', {})).ok, false)
    assert.deepEqual(first.requests.filter(request => request.method === 'prompts/get').map(request => request.params),
      [{ name: 'prompt_2', arguments: { topic: 'browser argument' } }])
  })
  await proof.prove('test', async () => {
    const result = await api('testMcpServer', server.id)
    assert.equal(result.ok, true)
    assert.deepEqual(result.tools, ['fixture_record', 'fixture_second'])
  })
  await proof.prove('logs', async () => {
    const log = await api('getMcpTestLog', server.id)
    assert.ok(log.some(entry => entry.msg === '[test] starting connection test'))
    assert.ok(log.some(entry => entry.msg === '[test] ok — 2 tools discovered'))
    await eventually(async () => {
      for (const page of [a, b]) assert.ok((await page.evaluate(() => window.__mcpProof.logs))
        .some(value => value.id === server.id && value.entry.msg === '[test] ok — 2 tools discovered'))
    })
  })
  await proof.prove('clear', async () => {
    for (const page of [a, b]) await page.evaluate(() => { window.__mcpProof.cleared = [] })
    await api('clearMcpTestLog', server.id)
    assert.deepEqual(await api('getMcpTestLog', server.id), [])
    await eventually(async () => {
      for (const page of [a, b]) assert.deepEqual(await page.evaluate(() => window.__mcpProof.cleared), [{ id: server.id }])
    })
  })
  await a.getByRole('button', { name: 'No folder session', exact: true }).click()
  let tab
  await eventually(async () => {
    tab = (await api('listTabs'))[0]
    assert.ok(tab?.conversationId)
  })
  await proof.prove('progress', async () => {
    const nonce = randomUUID()
    const result = await fixture.tool('fixture_record', { nonce })
    assert.deepEqual(result.result.content, [{ type: 'text', text: `first:${nonce}` }])
    await eventually(async () => {
      for (const page of [a, b]) assert.deepEqual((await page.evaluate(() => window.__mcpProof.progress)).filter(value => value.progressToken === nonce), [{
        serverId: server.id, serverName: 'browser_fixture', progressToken: nonce,
        progress: 1, total: 1, message: 'Synthetic tool complete',
      }])
    })
    assert.deepEqual((await readFile(first.journal, 'utf8')).trim().split('\n').map(line => JSON.parse(line)),
      [{ name: 'fixture_record', arguments: { nonce } }])
  })
  await proof.prove('update', async () => {
    const updated = await api('updateMcpServer', server.id, { name: 'renamed_fixture', url: second.url })
    assert.equal(updated.id, server.id)
    await eventually(async () => assert.deepEqual(await api('getMcpResources', server.id), second.resources))
    await eventually(async () => {
      const saved = await persisted()
      assert.equal(saved.browser_fixture, undefined)
      assert.equal(saved.renamed_fixture.url, second.url)
    })
    assert.equal(second.requests.filter(request => request.method === 'initialize').length, 1)
  })
  await proof.prove('toggle', async () => {
    await api('toggleMcpServer', server.id, false)
    assert.equal((await api('getMcpServers')).find(item => item.id === server.id).status, 'disconnected')
    assert.equal((await api('getMcpServers')).find(item => item.id === server.id).enabled, false)
    const count = second.requests.length
    assert.equal((await api('readMcpResource', server.id, second.resources[0].uri)).ok, false)
    assert.equal(second.requests.length, count, 'Disabled connector must not issue a request')
    await api('toggleMcpServer', server.id, true)
    assert.equal((await api('getMcpServers')).find(item => item.id === server.id).status, 'connected')
    assert.equal((await api('getMcpServers')).find(item => item.id === server.id).enabled, true)
    assert.equal(second.requests.filter(request => request.method === 'initialize').length, 2)
  })
  await proof.prove('zeroPage', async () => {
    await a.close(); await b.close()
    assert.equal(context.pages().length, 0)
    const nonce = randomUUID()
    const result = await fixture.tool('fixture_second', { nonce })
    assert.deepEqual(result.result.content, [{ type: 'text', text: `second:${nonce}` }])
    assert.deepEqual((await readFile(second.journal, 'utf8')).trim().split('\n').map(line => JSON.parse(line)),
      [{ name: 'fixture_second', arguments: { nonce } }])
    // Use the exact catalog sent by the host, not a locally reconstructed URI.
    await fixture.wait(frame => frame.type === 'mcp:register-external-content' && frame.resources?.some(item => item.rawUri === second.resources[1].uri))
    const catalog = fixture.receivedFrames().filter(frame => frame.type === 'mcp:register-external-content').at(-1)
    const resource = catalog.resources.find(item => item.rawUri === second.resources[1].uri)
    assert.ok(resource)
    const read = await fixture.tool('__bridge_mcp_resource_read', { uri: resource.uri })
    assert.deepEqual(read.result.contents, [{ uri: second.resources[1].uri,
      mimeType: 'text/plain', text: 'independently seeded MCP resource' }])
    assert.deepEqual(second.requests.filter(request => request.method === 'resources/read').map(request => request.params),
      [{ uri: second.resources[1].uri }])
    const prompt = catalog.prompts.find(item => item.rawName === 'prompt_2')
    assert.ok(prompt)
    const filled = await fixture.tool('__bridge_mcp_prompt_get', { name: prompt.name, arguments: { topic: nonce } })
    assert.equal(filled.result.messages[0].content.text, `second:${nonce}`)
    assert.deepEqual(second.requests.filter(request => request.method === 'prompts/get').map(request => request.params),
      [{ name: 'prompt_2', arguments: { topic: nonce } }])
    assert.equal(context.pages().length, 0)
    a = await open(); b = await open()
    assert.equal((await api('listTabs'))[0].conversationId, tab.conversationId)
    assert.equal((await api('getMcpServers')).find(item => item.id === server.id).name, 'renamed_fixture')
    assert.equal(second.requests.filter(request => request.method === 'tools/call').length, 1, 'Reattach must not replay tools')
    assert.equal(fixture.receivedFrames().filter(frame => ['session:create', 'session:resume'].includes(frame.type)).length, 1)
  })
  await proof.prove('remove', async () => {
    for (const page of [a, b]) await page.evaluate(() => { window.__mcpProof.servers = [] })
    await api('removeMcpServer', server.id)
    assert.deepEqual(await api('getMcpServers'), [])
    await eventually(async () => assert.equal((await persisted()).renamed_fixture, undefined))
    assert.equal((await api('readMcpResource', server.id, second.resources[0].uri)).ok, false)
    for (const page of [a, b]) await eventually(async () => assert.ok((await page.evaluate(() => window.__mcpProof.servers)).some(value => value.length === 0)))
  })
  const report = { kind: 'partial-browser-mcp-execution', upstream: 'synthetic', candidateImageTested: false,
    source: process.env.BRIDGEWEB_ACC_SOURCE || null, sourceDirty: process.env.BRIDGEWEB_ACC_SOURCE_DIRTY !== '0',
    platform: process.platform, browserVersion: browser.version(), ...proof.finish() }
  t.diagnostic(JSON.stringify(report))
  if (process.env.BRIDGEWEB_ACC_CONTAINER === '1') await writeFile('/reports/mcp-browser-checks.json', JSON.stringify(report, null, 2), { mode: 0o600 })
})
