// Collector integrity, never substitute these callbacks for host execution.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { monitorCases, monitorEvidence } from './monitor-evidence.mjs'

test('monitor proof IDs match the six supported browser API routes', async () => {
  const rows = (await Promise.all(['api', 'invoke', 'event'].map(async kind =>
    JSON.parse(await readFile(new URL(`../capabilities/${kind}.json`, import.meta.url)))))).flat()
  const ids = Object.values(monitorCases).flatMap(value => value.ids)
  assert.equal(new Set(ids).size, 12)
  for (const { ids } of Object.values(monitorCases)) {
    for (const id of ids) assert.equal(rows.find(row => row.id === id)?.status, 'supported-in-browser-host', id)
    assert.deepEqual(rows.find(row => row.id === ids[0]).routes, [ids[1]])
  }
})

test('monitor collector rejects unknown, missing, duplicate and failed assertions', async () => {
  const evidence = monitorEvidence()
  assert.throws(() => evidence.finish(), /incomplete/)
  await assert.rejects(evidence.prove('unreviewed', () => {}), /Unknown/)
  await assert.rejects(evidence.prove('list'), /Executed assertions/)
  await evidence.prove('list', () => {})
  await assert.rejects(evidence.prove('list', () => {}), /Duplicate/)
  await assert.rejects(evidence.prove('log', () => { throw new Error('synthetic assertion failure') }))
  for (const name of Object.keys(monitorCases).filter(name => !['list', 'log'].includes(name))) {
    await evidence.prove(name, () => {})
  }
  assert.throws(() => evidence.finish(), /failed/)
})

test('monitor collector waits for assertions and emits only fixed reviewed fields', async () => {
  assert.throws(() => { monitorCases.list.ids[0] = 'api:unreviewed' }, TypeError)
  assert.throws(() => { monitorCases.list.assertion = 'unreviewed' }, TypeError)
  const evidence = monitorEvidence()
  let release
  const pending = evidence.prove('list', () => new Promise(resolve => { release = resolve }))
  assert.throws(() => evidence.finish(), /incomplete/)
  release(); await pending
  for (const name of Object.keys(monitorCases).filter(name => name !== 'list')) await evidence.prove(name, () => {})
  const checks = evidence.finish()
  assert.equal(checks.length, 12)
  assert.ok(checks.every(check => check.status === 'PASS' && check.assertion.length > 20))
  assert.ok(checks.every(check => Object.keys(check).join(',') === 'id,status,test,assertion'))
  checks[0].status = 'FAIL'
  assert.equal(evidence.finish()[0].status, 'PASS')
})
