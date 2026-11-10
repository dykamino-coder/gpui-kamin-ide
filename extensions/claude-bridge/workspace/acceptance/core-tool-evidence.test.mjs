// Collector integrity only; these unit callbacks are not handler execution proof.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { coreToolCases, coreToolEvidence } from './core-tool-evidence.mjs'

test('reviewed core tool cases match independently maintained supported handler bindings', async () => {
  const tools = JSON.parse(await readFile(new URL('../capabilities/tool.json', import.meta.url)))
  const handlers = JSON.parse(await readFile(new URL('../capabilities/handler.json', import.meta.url)))
  for (const [tool, [handler]] of Object.entries(coreToolCases)) {
    const row = tools.find(row => row.id === `tool:${tool}`)
    assert.equal(row?.status, 'supported-in-browser-host')
    assert.equal(row.handler, handler)
    assert.equal(handlers.find(row => row.id === `handler:${handler}`)?.status, 'supported-in-browser-host')
  }
})

test('collector rejects missing, unknown, duplicate and failed assertions without generating a receipt', async () => {
  const evidence = coreToolEvidence()
  assert.throws(() => evidence.finish(), /incomplete/)
  await assert.rejects(evidence.prove('unreviewed', () => {}), /Unknown/)
  await assert.rejects(evidence.prove('Read'), /Executed assertions/)
  await evidence.prove('Read', async () => {})
  await assert.rejects(evidence.prove('Read', () => {}), /Duplicate/)
  await assert.rejects(evidence.prove('Write', async () => { throw new Error('synthetic assertion failure') }))
  for (const tool of Object.keys(coreToolCases).filter(tool => !['Read', 'Write'].includes(tool))) {
    await evidence.prove(tool, () => {})
  }
  assert.throws(() => evidence.finish(), /failed/)
})

test('collector waits for assertions and emits only fixed reviewed fields', async () => {
  const evidence = coreToolEvidence()
  let release
  const pending = evidence.prove('Read', () => new Promise(resolve => { release = resolve }))
  assert.throws(() => evidence.finish(), /incomplete/)
  release(); await pending
  for (const tool of Object.keys(coreToolCases).filter(tool => tool !== 'Read')) await evidence.prove(tool, () => {})
  const checks = evidence.finish()
  assert.equal(checks.length, 22)
  assert.equal(new Set(checks.map(check => check.id)).size, checks.length)
  assert.ok(checks.every(check => check.status === 'PASS' && check.assertion.length > 20))
  checks[0].status = 'FAIL'
  assert.equal(evidence.finish()[0].status, 'PASS', 'Callers cannot mutate collector state')
})
