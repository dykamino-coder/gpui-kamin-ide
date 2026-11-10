// Real browser/host/shared handlers; synthetic upstream and stdio language peer.
import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'
import { pathToFileURL } from 'node:url'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { lspEvidence } from '../acceptance/lsp-evidence.mjs'
import { previewFixture } from './fixtures/preview-fixture.mjs'

async function installPeer({ root, home }) {
  const plugin = path.join(home, '.claude', 'plugins', 'cache', 'fixture')
  await mkdir(path.join(plugin, '.claude-plugin'), { recursive: true })
  await copyFile(new URL('./fixtures/lsp-server.cjs', import.meta.url), path.join(plugin, 'server.cjs'))
  const json = (file, value) => writeFile(file, JSON.stringify(value))
  await json(path.join(plugin, '.claude-plugin', 'plugin.json'), { name: 'fixture', lspServers: {
    fixture: { command: 'node', args: ['${CLAUDE_PLUGIN_ROOT}/server.cjs'],
      extensionToLanguage: { '.proof': 'plaintext' },
      env: { BRIDGEWEB_LSP_TRACE: path.join(root, 'lsp-trace.jsonl') }, restartOnCrash: false },
  } })
  await json(path.join(home, '.claude', 'plugins', 'installed_plugins.json'), { version: 2,
    plugins: { 'fixture@local': [{ installPath: plugin }] } })
  await json(path.join(home, '.claude', 'settings.json'), { enabledPlugins: { 'fixture@local': true } })
}

async function eventually(read, assertion, label) {
  const deadline = Date.now() + 10000
  for (;;) {
    const value = await read()
    if (assertion(value)) return value
    assert.ok(Date.now() < deadline, label)
    await sleep(25)
  }
}

async function stopped(pid) {
  await eventually(() => {
    try { process.kill(pid, 0); return false }
    catch (error) { assert.equal(error.code, 'ESRCH'); return true }
  }, value => value, 'Owned LSP process did not exit')
}

test('plugin LSP browser discovery, zero-page tools, restart and project process ownership', { timeout: 90000 }, async t => {
  const fixture = await previewFixture(t, { conversations: true, prepare: installPeer })
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
    return page
  }
  let page = await open()
  const initialList = await page.evaluate(() => window.kaminBridge.listLspServers())
  const evidence = lspEvidence()
  async function session(cwd) {
    const id = await page.evaluate(async cwd => {
      const config = await window.kaminBridge.getConfig()
      return window.kaminBridge.createTab({ ...config, cwd })
    }, cwd)
    return eventually(async () => (await page.evaluate(() => window.kaminBridge.listTabs())).find(tab => tab.id === id),
      tab => !!tab?.conversationId, 'Browser-created session did not establish')
  }
  const otherRoot = path.join(fixture.root, 'other-project')
  await mkdir(otherRoot)
  const first = await session(fixture.root), shared = await session(fixture.root), other = await session(otherRoot)
  const file = path.join(fixture.root, 'example.proof'), otherFile = path.join(otherRoot, 'other.proof')
  await writeFile(file, 'independent original document')
  await writeFile(otherFile, 'isolated project document')
  const records = async () => (await readFile(path.join(fixture.root, 'lsp-trace.jsonl'), 'utf8'))
    .trim().split('\n').map(line => JSON.parse(line))
  const initializes = async () => (await records()).filter(row => row.method === 'initialize')
  const uri = pathToFileURL(file).href
  const position = { line: 1, character: 2 }
  async function tool(tab, name, target = file) {
    assert.equal(context.pages().length, 0, 'Tool assertion requires zero pages')
    const response = await fixture.tool(name, { file: target, ...position }, tab.conversationId)
    const text = response.result.content.map(item => item.text || '').join('\n')
    assert.doesNotMatch(text, /^Error:/)
    return text
  }
  await page.close()
  let firstPid
  await evidence.prove('hover', async () => {
    assert.equal(await tool(first, 'LspHover'), `Hover at ${file}:2:3\n\nfixture hover: independent original document`)
    await writeFile(file, 'independent changed document')
    assert.equal(await tool(shared, 'LspHover'), `Hover at ${file}:2:3\n\nfixture hover: independent changed document`)
    const rows = await records(), init = await initializes()
    assert.equal(init.length, 1, 'Same-project tabs share one LSP process')
    firstPid = init[0].pid
    assert.equal(init[0].cwd, fixture.root)
    assert.equal(init[0].params.rootUri, pathToFileURL(fixture.root).href)
    assert.equal(rows.find(row => row.method === 'textDocument/didOpen').params.textDocument.text, 'independent original document')
    const changed = rows.find(row => row.method === 'textDocument/didChange')
    assert.equal(changed.params.textDocument.uri, uri)
    assert.equal(changed.params.textDocument.version, 2)
    assert.deepEqual(changed.params.contentChanges, [{ text: 'independent changed document' }])
    assert.deepEqual(rows.filter(row => row.method === 'textDocument/hover').map(row => row.params),
      [{ textDocument: { uri }, position }, { textDocument: { uri }, position }])
  })
  await evidence.prove('definition', async () => {
    assert.equal(await tool(first, 'LspDefinition'), `${uri}:2:3`)
    assert.deepEqual((await records()).find(row => row.method === 'textDocument/definition').params,
      { textDocument: { uri }, position })
  })
  await evidence.prove('references', async () => {
    assert.equal(await tool(first, 'LspReferences'), `${uri}:2:3`)
    assert.deepEqual((await records()).find(row => row.method === 'textDocument/references').params,
      { textDocument: { uri }, position, context: { includeDeclaration: true } })
  })
  await evidence.prove('diagnostics', async () => {
    assert.equal(await tool(shared, 'LspDiagnostics'),
      `[Warning] ${file}:2:3 independent changed document (workspace-fixture) [fixture-warning]`)
  })
  assert.match(await tool(other, 'LspHover', otherFile), /fixture hover: isolated project document/)
  const otherInit = (await initializes()).find(row => row.cwd === otherRoot)
  assert.ok(otherInit)
  assert.notEqual(otherInit.pid, firstPid)
  assert.equal(otherInit.params.rootUri, pathToFileURL(otherRoot).href)
  page = await open()
  await evidence.prove('list', async () => {
    assert.deepEqual(initialList, [{ id: 'fixture@local/fixture', pluginId: 'fixture@local',
      serverKey: 'fixture', extensions: ['.proof'], initialized: false }])
    assert.deepEqual(await page.evaluate(() => window.kaminBridge.listLspServers()),
      initialList.map(row => ({ ...row, initialized: true })))
    assert.equal((await initializes()).length, 2)
  })
  let restartedFirst, restartedOther
  await evidence.prove('restart', async () => {
    assert.deepEqual(await page.evaluate(() => window.kaminBridge.restartLspServers()), { ok: true })
    await stopped(firstPid); await stopped(otherInit.pid)
    assert.deepEqual(await page.evaluate(() => window.kaminBridge.listLspServers()), initialList)
    await page.close()
    assert.match(await tool(first, 'LspHover'), /fixture hover: independent changed document/)
    assert.match(await tool(shared, 'LspHover'), /fixture hover: independent changed document/)
    assert.match(await tool(other, 'LspHover', otherFile), /fixture hover: isolated project document/)
    const init = await initializes()
    assert.equal(init.length, 4, 'Restart creates one new peer per project')
    restartedFirst = init[2].pid; restartedOther = init[3].pid
    assert.equal(new Set(init.map(row => row.pid)).size, 4)
    assert.deepEqual(init.slice(2).map(row => [row.cwd, row.params.rootUri]),
      [[fixture.root, pathToFileURL(fixture.root).href], [otherRoot, pathToFileURL(otherRoot).href]])
    const trace = await records()
    for (const pid of [firstPid, otherInit.pid]) assert.ok(trace.some(row => row.pid === pid && row.method === 'shutdown'))
  })
  page = await open()
  await page.evaluate(id => window.kaminBridge.closeTab(id), first.id)
  await eventually(() => page.evaluate(() => window.kaminBridge.listTabs()), tabs => !tabs.some(tab => tab.id === first.id), 'First tab did not close')
  await page.close()
  assert.match(await tool(shared, 'LspHover'), /independent changed document/)
  assert.doesNotThrow(() => process.kill(restartedFirst, 0))
  page = await open()
  await page.evaluate(id => window.kaminBridge.closeTab(id), shared.id)
  await eventually(() => page.evaluate(() => window.kaminBridge.listTabs()), tabs => !tabs.some(tab => tab.id === shared.id), 'Last project tab did not close')
  await stopped(restartedFirst)
  await page.close()
  assert.match(await tool(other, 'LspHover', otherFile), /isolated project document/)
  assert.doesNotThrow(() => process.kill(restartedOther, 0))
  assert.deepEqual(await fixture.shutdown(), { code: 0, signal: null, forced: false }, 'Host must exit naturally with zero pages')
  await stopped(restartedOther)
  assert.equal((await initializes()).length, 4)
  assert.equal(fixture.receivedFrames().filter(frame => ['session:create', 'session:resume'].includes(frame.type)).length, 3)
  const report = { kind: 'partial-plugin-lsp-execution', upstream: 'synthetic', languageServer: 'synthetic-stdio',
    candidateImageTested: false, source: process.env.BRIDGEWEB_ACC_SOURCE,
    sourceDirty: process.env.BRIDGEWEB_ACC_SOURCE_DIRTY !== '0', platform: process.platform,
    browserVersion: browser.version(), checks: evidence.finish() }
  t.diagnostic(JSON.stringify(report))
  if (process.env.BRIDGEWEB_ACC_CONTAINER === '1') {
    await writeFile('/reports/lsp-checks.json', JSON.stringify(report, null, 2) + '\n', { mode: 0o600 })
  }
})
