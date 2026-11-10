// Actual shared client handlers with zero pages; synthetic upstream, no model.
import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'
import { coreToolEvidence } from '../acceptance/core-tool-evidence.mjs'
import { previewFixture } from './fixtures/preview-fixture.mjs'

test('eleven shared core tools execute and retain results with every browser page closed', { timeout: 90000 }, async t => {
  const fixture = await previewFixture(t, { conversations: true })
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
  await page.getByRole('button', { name: 'No folder session', exact: true }).click()
  let tab
  const deadline = Date.now() + 20000
  while (!(tab = (await page.evaluate(() => window.kaminBridge.listTabs()))[0])?.conversationId) {
    assert.ok(Date.now() < deadline, 'Shared session identity timed out')
    await sleep(50)
  }
  await page.close()
  assert.equal(context.pages().length, 0)
  const evidence = coreToolEvidence()
  const target = path.join(fixture.root, 'core.txt')
  const unrelated = path.join(fixture.root, 'excluded.md')
  await writeFile(unrelated, 'unrelated text\n')
  const tool = async (name, input) => {
    assert.equal(context.pages().length, 0, 'Behavioral assertions must execute without a page')
    const response = await fixture.tool(name, input)
    const text = response.result.content.filter(item => item.type === 'text').map(item => item.text).join('\n')
    assert.doesNotMatch(text, /^Error:/, 'Shared handler must not return an error disguised as text')
    return text
  }
  await evidence.prove('Write', async () => {
    await tool('Write', { file_path: target, content: 'before\nuntouched\n' })
    assert.equal(await readFile(target, 'utf8'), 'before\nuntouched\n')
  })
  await evidence.prove('Edit', async () => {
    await tool('Edit', { file_path: target, old_string: 'before', new_string: 'core-edited-marker' })
    assert.equal(await readFile(target, 'utf8'), 'core-edited-marker\nuntouched\n')
  })
  await evidence.prove('Read', async () => {
    const seeded = path.join(fixture.root, 'seeded.txt')
    await writeFile(seeded, 'independently-seeded-marker\n')
    assert.match(await tool('Read', { file_path: seeded }), /independently-seeded-marker/)
  })
  await evidence.prove('Glob', async () => {
    const found = await tool('Glob', { path: fixture.root, pattern: '*.txt' })
    assert.ok(found.includes(target))
    assert.ok(!found.includes(unrelated))
  })
  await evidence.prove('Grep', async () => {
    const found = await tool('Grep', { path: fixture.root, pattern: 'core-edited-marker', glob: '*.txt', output_mode: 'content' })
    assert.match(found, /core\.txt:\s*1:core-edited-marker/)
    assert.doesNotMatch(found, /unrelated text|independently-seeded-marker/)
  })
  await evidence.prove('Bash', async () => {
    const result = await tool('Bash', { command: `printf core-stdout; printf core-stderr >&2; printf shell-file > '${fixture.root}/shell.txt'`, timeout: 1000 })
    assert.match(result, /core-stdout/)
    assert.match(result, /core-stderr/)
    assert.equal(await readFile(path.join(fixture.root, 'shell.txt'), 'utf8'), 'shell-file')
  })
  const started = await tool('Bash', { command: 'printf bg-stdout; printf bg-stderr >&2; exec sleep 120', run_in_background: true })
  const match = started.match(/bash_id: (\d+), pid: (\d+)/)
  assert.ok(match, 'Background shell must identify its own process')
  const bashId = Number(match[1]), pid = Number(match[2])
  await evidence.prove('BashOutput', async () => {
    let output = ''
    const deadline = Date.now() + 10000
    while (!output.includes('bg-stdout') || !output.includes('bg-stderr')) {
      output += await tool('BashOutput', { bash_id: bashId })
      assert.ok(Date.now() < deadline, 'Background output timed out')
      await sleep(25)
    }
    assert.match(output, /Status: running/)
    const consumed = await tool('BashOutput', { bash_id: bashId })
    assert.doesNotMatch(consumed, /bg-stdout|bg-stderr/)
    assert.match(consumed, /no new output/)
  })
  await evidence.prove('KillShell', async () => {
    assert.match(await tool('KillShell', { bash_id: bashId }), /Killed background shell/)
    const deadline = Date.now() + 10000
    for (;;) {
      try { process.kill(pid, 0) }
      catch (error) { assert.equal(error.code, 'ESRCH'); break }
      assert.ok(Date.now() < deadline, 'Own background process did not exit')
      await sleep(25)
    }
    assert.match(await tool('BashOutput', { bash_id: bashId }), /Status: exited/)
  })
  await evidence.prove('NotebookEdit', async () => {
    const notebook = path.join(fixture.root, 'core.ipynb')
    await writeFile(notebook, JSON.stringify({ nbformat: 4, nbformat_minor: 5, metadata: {}, cells: [
      { id: 'original', cell_type: 'code', source: ['before'], metadata: {}, outputs: [], execution_count: null },
    ] }))
    const cells = async () => JSON.parse(await readFile(notebook, 'utf8')).cells
    await tool('NotebookEdit', { notebook_path: notebook, cell_id: 'original', new_source: 'after\nsecond', edit_mode: 'replace' })
    assert.equal((await cells())[0].source.join(''), 'after\nsecond')
    await tool('NotebookEdit', { notebook_path: notebook, cell_number: 1, cell_type: 'markdown', new_source: 'inserted', edit_mode: 'insert' })
    assert.deepEqual((await cells()).map(cell => cell.cell_type), ['code', 'markdown'])
    assert.equal((await cells())[1].source.join(''), 'inserted')
    await tool('NotebookEdit', { notebook_path: notebook, cell_number: 1, new_source: '', edit_mode: 'delete' })
    assert.equal((await cells()).length, 1)
    assert.equal((await cells())[0].id, 'original')
    assert.equal((await cells())[0].source.join(''), 'after\nsecond')
  })
  const todos = [{ content: 'zero-page-todo', status: 'in_progress', activeForm: 'Checking tools' }]
  await evidence.prove('TodoWrite', async () => {
    assert.match(await tool('TodoWrite', { todos }), /Todo list updated \(1 item\):\n1\. \[in_progress\] zero-page-todo/)
  })
  await evidence.prove('TodoList', async () => {
    assert.deepEqual(JSON.parse(await tool('TodoList', {})), todos)
    await tool('TodoWrite', { todos: [] })
    assert.equal(await tool('TodoList', {}), 'No todos.')
  })
  page = await open()
  assert.equal((await page.evaluate(() => window.kaminBridge.listTabs()))[0].id, tab.id)
  const base64 = await page.evaluate(file => window.kaminBridge.readFileAsBase64(file), target)
  assert.equal(base64.ok, true)
  assert.equal(Buffer.from(base64.base64, 'base64').toString(), 'core-edited-marker\nuntouched\n')
  assert.equal(fixture.receivedFrames().filter(frame => ['session:create', 'session:resume'].includes(frame.type)).length, 1)
  const report = { kind: 'partial-core-tool-execution', upstream: 'synthetic', candidateImageTested: false,
    source: process.env.BRIDGEWEB_ACC_SOURCE, sourceDirty: process.env.BRIDGEWEB_ACC_SOURCE_DIRTY !== '0',
    platform: process.platform, browserVersion: browser.version(), checks: evidence.finish() }
  t.diagnostic(JSON.stringify(report))
  if (process.env.BRIDGEWEB_ACC_CONTAINER === '1') {
    await writeFile('/reports/core-tool-checks.json', JSON.stringify(report, null, 2) + '\n', { mode: 0o600 })
  }
})
