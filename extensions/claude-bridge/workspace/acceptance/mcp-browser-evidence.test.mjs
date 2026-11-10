import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { mcpBrowserCases, mcpBrowserEvidence } from './mcp-browser-evidence.mjs'

test('MCP partial case mapping retains unique supported API IDs and their exact routes', async () => {
  const rows = (await Promise.all(['api', 'invoke', 'event'].map(async kind =>
    JSON.parse(await readFile(new URL(`../capabilities/${kind}.json`, import.meta.url), 'utf8'))))).flat()
  const byId = new Map(rows.map(row => [row.id, row]))
  const ids = Object.values(mcpBrowserCases).flat()
  assert.equal(new Set(ids).size, ids.length)
  assert.equal(ids.length, 34)
  for (const group of Object.values(mcpBrowserCases)) for (const id of group) {
    assert.equal(byId.get(id)?.status, 'supported-in-browser-host', `Unmapped supported capability: ${id}`)
    if (id.startsWith('api:')) assert.deepEqual(byId.get(id).routes, group.filter(route => !route.startsWith('api:')).filter(route =>
      // Each API maps to its own route, even when several assertions share a case.
      byId.get(id).routes.includes(route)))
  }
})

test('MCP evidence rejects missing, unknown, duplicate and failed assertion callbacks', async () => {
  const evidence = mcpBrowserEvidence()
  assert.throws(() => evidence.finish(), /Missing/)
  await assert.rejects(evidence.prove('unknown', async () => {}), /Unknown/)
  await assert.rejects(evidence.prove('status'), /callback/)
  await assert.rejects(evidence.prove('status', async () => { throw new Error('assertion failed') }), /assertion failed/)
  await assert.rejects(evidence.prove('status', async () => {}), /Duplicate/)
  for (const name of Object.keys(mcpBrowserCases).filter(name => name !== 'status')) await evidence.prove(name, async () => {})
  assert.throws(() => evidence.finish(), /Missing or failed/)
})

test('MCP evidence records each case only after its assertions settle', async () => {
  const evidence = mcpBrowserEvidence()
  let release
  const pending = evidence.prove('status', () => new Promise(resolve => { release = resolve }))
  for (const name of Object.keys(mcpBrowserCases).filter(name => name !== 'status')) await evidence.prove(name, async () => {})
  assert.throws(() => evidence.finish(), /Missing/)
  release(); await pending
  const report = evidence.finish()
  assert.equal(report.cases.length, 14)
  assert.equal(report.checks.length, 34)
  assert.ok(report.checks.every(check => check.status === 'PASS' && check.assertion && check.test))
  assert.equal(report.schemaVersion, undefined, 'Partial checks are not a completion receipt')
})
