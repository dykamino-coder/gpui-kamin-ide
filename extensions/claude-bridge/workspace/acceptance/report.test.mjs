import { test } from 'node:test'
import assert from 'node:assert/strict'
import { CheckFailure, outcome, exitStatus, markdown, assertImageSource, completionReceipt, scenarios } from './report.mjs'
import { complete, used, turn } from './browser.mjs'
import { spawn } from 'node:child_process'
import readline from 'node:readline'
import { fileURLToPath } from 'node:url'

test('live acceptance rejects dirty runners and images from another source before execution', () => {
  const source = 'a'.repeat(40)
  const candidate = { source, sourceDirty: false, imageSource: source }
  assertImageSource(candidate)
  for (const imageSource of [undefined, 'unspecified', 'a'.repeat(8), 'b'.repeat(40)]) {
    assert.throws(() => assertImageSource({ ...candidate, imageSource }), { code: 'IMAGE_SOURCE_MISMATCH' })
  }
  for (const sourceDirty of [true, undefined]) {
    assert.throws(() => assertImageSource({ ...candidate, sourceDirty }), { code: 'RUNNER_SOURCE_DIRTY' })
  }
  assert.throws(() => assertImageSource({ ...candidate, source: '', imageSource: '' }), { code: 'IMAGE_SOURCE_MISMATCH' })
})

test('completion receipt preserves actual live outcomes and cannot invent capability or lifecycle proof', () => {
  const report = { source: 'a'.repeat(40), sourceDirty: false, imageSource: 'a'.repeat(40),
    imageId: 'b'.repeat(64), metrics: { browser: 'synthetic-browser' }, exitCode: 1,
    cleanup: ['OWN_RESOURCE_CLEANUP'],
    results: scenarios.map((name, index) => ({ name, status: ['PASS', 'FAIL', 'BLOCKED', 'XFAIL'][index % 4], reason: 'synthetic-private-text' })) }
  const receipt = completionReceipt(report)
  assert.equal(receipt.checks.length, scenarios.length)
  assert.deepEqual(receipt.checks.map(check => check.status), report.results.map(result => result.status))
  assert.ok(receipt.checks.every(check => check.id.startsWith('live:') && check.test && check.assertion))
  assert.equal(receipt.source, report.source)
  assert.equal(receipt.imageSource, report.imageSource)
  assert.equal(receipt.imageId, report.imageId)
  assert.equal(receipt.exitCode, 1)
  assert.deepEqual(receipt.cleanup, ['OWN_RESOURCE_CLEANUP'])
  assert.ok(!JSON.stringify(receipt).includes('synthetic-private-text'))
  assert.deepEqual(completionReceipt({ ...report, results: [] }).checks, [])
})

test('known feature gaps never hide unrelated exceptions or convert blocked prerequisites to success', () => {
  const pending = { 'file-panel': { FILE_PANEL: 'Fixture dependency is pending.' } }
  assert.equal(outcome('file-panel', new CheckFailure('FILE_PANEL'), false, pending).status, 'XFAIL')
  assert.equal(outcome('file-panel', new CheckFailure('FILE_PANEL'), true, pending).status, 'FAIL')
  assert.equal(outcome('file-panel', new CheckFailure('MODEL_TIMEOUT'), false, pending).status, 'FAIL')
  assert.equal(outcome('file-panel', new CheckFailure('FILE_PANEL')).status, 'FAIL', 'Landed features must not retain stale XFAIL allowances')
  assert.equal(outcome('file-panel').status, 'PASS')
  assert.equal(exitStatus([{ status: 'PASS' }, { status: 'XFAIL' }, { status: 'BLOCKED' }]), 2)
  assert.equal(exitStatus([{ status: 'FAIL' }, { status: 'BLOCKED' }]), 1)
})

test('exception text cannot leak into a report, and IDE exclusion keeps the owner scope reason', () => {
  const secret = 'synthetic-not-a-real-credential'
  const result = outcome('chat', new Error(`Authorization ${secret}; transcript ${secret}`))
  const report = { started: 'fixture', source: 'fixture', results: [result], metrics: {} }
  assert.ok(!JSON.stringify(report).includes(secret))
  assert.ok(!markdown(report).includes(secret))
  assert.match(markdown(report), /NOT-APPLICABLE \| KaminIDE host feature, not Bridge/)
})

test('turn wait failures identify the failed stage without leaking upstream error text', async t => {
  t.mock.method(console, 'log', () => {})
  const secret = 'synthetic-not-a-real-credential'
  for (const stage of ['entry', 'idle']) {
    let evaluations = 0
    const page = {
      async evaluate() {
        evaluations++
        if (evaluations === 1) return 'fixture-tab'
        // Also exercise loss of the page while collecting diagnostic state.
        throw new Error(secret)
      },
      async waitForFunction() { if (stage === 'entry') throw new Error(secret) },
    }
    await assert.rejects(turn(page, 'Reply acceptance-aaaaaaaa:ok'), error => {
      assert.equal(error.code, stage === 'entry' ? 'TURN_ENTRY_WAIT' : 'TURN_IDLE_WAIT')
      assert.ok(!error.message.includes(secret))
      return true
    })
  }
})

test('persistence assertions reject duplicate/missing transcript entries and incomplete tool calls', () => {
  const valid = { ids: ['user', 'tool', 'result', 'assistant'], duplicateIds: 0, users: 1, done: true, unresolved: 0,
    tools: ['mcp__bridge__Read'] }
  complete(valid, { baseline: ['user'] })
  used(valid, 'Read')
  for (const invalid of [{ duplicateIds: 1 }, { users: 2 }, { done: false }, { unresolved: 1 }]) {
    assert.throws(() => complete({ ...valid, ...invalid }, { code: 'SNAPSHOT' }), { code: 'SNAPSHOT' })
  }
  assert.throws(() => complete(valid, { baseline: ['missing'], code: 'SNAPSHOT' }), { code: 'SNAPSHOT' })
})

test('stdio fixture speaks MCP initialize and discovery over the actual process transport', async () => {
  const child = spawn(process.execPath, [fileURLToPath(new URL('./mcp-fixture.mjs', import.meta.url))], {
    windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'],
  })
  const responses = []
  const closed = new Promise(resolve => child.once('close', resolve))
  const lines = readline.createInterface({ input: child.stdout })
  lines.on('line', line => responses.push(JSON.parse(line)))
  try {
    child.stdin.end([
      { jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2024-11-05' } },
      { jsonrpc: '2.0', method: 'notifications/initialized' },
      { jsonrpc: '2.0', id: 2, method: 'tools/list' },
    ].map(value => JSON.stringify(value)).join('\n') + '\n')
    assert.equal(await closed, 0)
    assert.deepEqual(responses.map(r => r.id), [1, 2])
    assert.equal(responses[0].result.protocolVersion, '2024-11-05')
    assert.equal(responses[1].result.tools[0].name, 'acceptance_record')
  } finally { lines.close(); if (child.exitCode === null) child.kill() }
})
