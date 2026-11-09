import assert from 'node:assert/strict'
import { test } from 'node:test'
import { capabilityInventory } from './capability-inventory.js'
import { readMatrix } from './capability-matrix.js'
import { assertFullParity, completionScenarios, fullParityGaps, requiredProofIds, type CompletionReceipt } from './full-parity.js'

const inventory = capabilityInventory()
const matrix = readMatrix()
const source = 'a'.repeat(40)
// Schema-validation fixture only. It is not executed product acceptance evidence.
function fixture(): CompletionReceipt {
  return { schemaVersion: 1, source, sourceDirty: false, imageSource: source, imageId: 'b'.repeat(64),
    platform: 'linux', browserVersion: 'synthetic-browser', exitCode: 0, cleanup: [],
    checks: requiredProofIds(matrix).map(id => ({ id, status: 'PASS', test: 'synthetic validator fixture', assertion: 'synthetic observable assertion' })) }
}
const gaps = (receipt: unknown) => fullParityGaps(inventory, matrix, receipt, source, false)

test('full completion remains blocked despite passing declared provider prerequisites', () => {
  const pending = gaps(undefined)
  for (const row of matrix.rows.filter(row => row.status === 'supported-in-browser-host')) {
    assert.ok(pending.includes(`MISSING capability:${row.id}`), row.id)
  }
  assert.ok(pending.includes('MISSING contract:host-transcript-reducers'))
  assert.ok(pending.includes('MISSING contract:host-message-queue'))
  assert.ok(pending.includes('MISSING live:restart-persistence'))
  assert.throws(() => assertFullParity(inventory, matrix, undefined, source, false), /Full browser parity remains OPEN/)
})

test('actual live receipt producer supplies only its bounded scenario proof and still fails full completion', async () => {
  const producer: { scenarios: string[]; completionReceipt: (report: unknown) => CompletionReceipt } =
    await import(new URL('../acceptance/report.mjs', import.meta.url).href)
  for (const name of producer.scenarios) assert.ok(completionScenarios.some(id => id === name), `Unreviewed live scenario ${name}`)
  const receipt = producer.completionReceipt({ source, sourceDirty: false, imageSource: source,
    imageId: 'b'.repeat(64), metrics: { browser: 'synthetic-browser' }, cleanup: [], exitCode: 0,
    results: producer.scenarios.map(name => ({ name, status: 'PASS' })) })
  const missing = gaps(receipt)
  for (const name of producer.scenarios) assert.ok(!missing.includes(`MISSING live:${name}`))
  assert.ok(missing.includes('MISSING live:restart-persistence'))
  assert.ok(missing.includes('MISSING contract:host-message-queue'))
  assert.ok(missing.some(gap => gap.startsWith('MISSING capability:')))
  receipt.checks.find(check => check.id === 'live:chat')!.status = 'BLOCKED'
  assert.ok(gaps(receipt).includes('UNPROVEN live:chat'))
})

test('schema fixture covers every required ID; each independently missing or unexecuted assertion rejects completion', () => {
  assert.deepEqual(gaps(fixture()), [])
  for (const id of requiredProofIds(matrix)) {
    const receipt = fixture()
    receipt.checks = receipt.checks.filter(check => check.id !== id)
    assert.ok(gaps(receipt).includes(`MISSING ${id}`), id)
  }
  for (const status of ['FAIL', 'BLOCKED', 'XFAIL', 'SKIP'] as const) {
    const receipt = fixture()
    receipt.checks[0].status = status
    assert.ok(gaps(receipt).includes(`UNPROVEN ${receipt.checks[0].id}`))
  }
  for (const field of ['test', 'assertion'] as const) {
    const receipt = fixture()
    receipt.checks[0][field] = ' '
    assert.ok(gaps(receipt).includes(`UNPROVEN ${receipt.checks[0].id}`))
  }
})

test('proof cannot hide new inventory, duplicate assertions, native exclusions or deleted completion floors', () => {
  const receipt = fixture()
  receipt.checks.push(receipt.checks[0])
  assert.ok(gaps(receipt).some(gap => gap.startsWith('DUPLICATE ')))
  receipt.checks.push({ id: 'capability:api:openInVscode', status: 'PASS', test: 'fixture', assertion: 'fixture' })
  assert.ok(gaps(receipt).includes('UNKNOWN_PROOF_ID'))
  const changed = structuredClone(matrix)
  changed.rows.pop()
  assert.throws(() => assertFullParity(inventory, changed, fixture(), source, false), /Unmapped\/stale/)
  const noContracts = fixture()
  noContracts.checks = noContracts.checks.filter(check => !check.id.startsWith('contract:'))
  assert.equal(gaps(noContracts).filter(gap => gap.startsWith('MISSING contract:')).length, 7)
})

test('old images, dirty runners, failed cleanup and malformed artifacts never pass or echo private input', () => {
  const mutate = (change: (receipt: Record<string, unknown>) => void, expected: string) => {
    const receipt = fixture() as unknown as Record<string, unknown>
    change(receipt)
    assert.ok(gaps(receipt).includes(expected), expected)
  }
  mutate(r => { r.source = 'c'.repeat(40) }, 'RECEIPT_SOURCE_MISMATCH')
  mutate(r => { r.imageSource = 'c'.repeat(40) }, 'RECEIPT_SOURCE_MISMATCH')
  mutate(r => { r.sourceDirty = true }, 'RECEIPT_SOURCE_MISMATCH')
  mutate(r => { r.imageId = 'moving-tag' }, 'IMMUTABLE_IMAGE_REQUIRED')
  mutate(r => { r.platform = 'win32' }, 'LINUX_BROWSER_EXECUTION_REQUIRED')
  mutate(r => { r.browserVersion = '' }, 'LINUX_BROWSER_EXECUTION_REQUIRED')
  mutate(r => { r.exitCode = 1 }, 'EXECUTION_OR_CLEANUP_FAILED')
  mutate(r => { r.cleanup = ['synthetic-private-text'] }, 'EXECUTION_OR_CLEANUP_FAILED')
  mutate(r => { r.schemaVersion = 2 }, 'RECEIPT_SCHEMA')
  mutate(r => { r.checks = null }, `MISSING ${requiredProofIds(matrix)[0]}`)
  assert.ok(fullParityGaps(inventory, matrix, fixture(), source, true).includes('CANDIDATE_MUST_BE_CLEAN_COMMIT'))
  assert.ok(fullParityGaps(inventory, matrix, fixture(), 'short', false).includes('CANDIDATE_MUST_BE_CLEAN_COMMIT'))
  const receipt = fixture()
  receipt.checks.push({ id: 'synthetic-private-text', status: 'FAIL', test: 'private', assertion: 'private' })
  assert.ok(!JSON.stringify(gaps(receipt)).includes('synthetic-private-text'))
  for (const malformed of [null, [], 'synthetic-private-text', 1]) assert.ok(gaps(malformed).includes('EXECUTION_RECEIPT_MISSING'))
})
