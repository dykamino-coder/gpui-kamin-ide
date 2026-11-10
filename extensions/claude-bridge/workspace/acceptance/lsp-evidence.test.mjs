// Collector integrity, never substitute these callbacks for handler execution.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { lspCases, lspEvidence } from './lsp-evidence.mjs'

test('LSP proof IDs match supported inventory routes and tool bindings', async () => {
  const rows = (await Promise.all(['api', 'invoke', 'tool', 'handler'].map(async kind =>
    JSON.parse(await readFile(new URL(`../capabilities/${kind}.json`, import.meta.url)))))).flat()
  const ids = Object.values(lspCases).flatMap(value => value.ids)
  assert.equal(new Set(ids).size, 12)
  for (const { ids } of Object.values(lspCases)) {
    for (const id of ids) assert.equal(rows.find(row => row.id === id)?.status, 'supported-in-browser-host', id)
    const row = rows.find(row => row.id === ids[0])
    if (ids[0].startsWith('api:')) assert.deepEqual(row.routes, [ids[1]])
    else assert.equal(row.handler, ids[1].slice('handler:'.length))
  }
})

test('LSP collector rejects unknown, missing, duplicate and failed cases', async () => {
  const evidence = lspEvidence()
  assert.throws(() => evidence.finish(), /incomplete/)
  await assert.rejects(evidence.prove('unreviewed', () => {}), /Unknown/)
  await assert.rejects(evidence.prove('list'), /Executed assertions/)
  await evidence.prove('list', () => {})
  await assert.rejects(evidence.prove('list', () => {}), /Duplicate/)
  await assert.rejects(evidence.prove('hover', () => { throw new Error('synthetic assertion failure') }))
  for (const name of Object.keys(lspCases).filter(name => !['list', 'hover'].includes(name))) {
    await evidence.prove(name, () => {})
  }
  assert.throws(() => evidence.finish(), /failed/)
})

test('LSP collector waits for assertions and returns only fixed immutable-state fields', async () => {
  assert.throws(() => { lspCases.list.ids[0] = 'api:unreviewed' }, TypeError)
  assert.throws(() => { lspCases.list.assertion = 'unreviewed' }, TypeError)
  const evidence = lspEvidence()
  let release
  const pending = evidence.prove('list', () => new Promise(resolve => { release = resolve }))
  assert.throws(() => evidence.finish(), /incomplete/)
  release(); await pending
  for (const name of Object.keys(lspCases).filter(name => name !== 'list')) await evidence.prove(name, () => {})
  const checks = evidence.finish()
  assert.equal(checks.length, 12)
  assert.ok(checks.every(check => check.status === 'PASS' && check.assertion.length > 20))
  assert.ok(checks.every(check => Object.keys(check).join(',') === 'id,status,test,assertion'))
  checks[0].status = 'FAIL'
  assert.equal(evidence.finish()[0].status, 'PASS')
})
