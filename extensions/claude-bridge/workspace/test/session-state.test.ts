import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { WorkspaceSessionState } from '../src/session-state.js'

test('restart disk state is atomic, credential-free, scoped and rejects corrupt input', t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-state-'))
  t.after(() => {
    assert.equal(path.dirname(directory), fs.realpathSync(os.tmpdir()))
    assert.ok(path.basename(directory).startsWith('bridgeweb-state-'))
    fs.rmSync(directory, { recursive: true })
  })
  const store = (root = '/workspace', url = 'ws://bridge', token = 'synthetic-token') => new WorkspaceSessionState(directory, root, url, token)
  const tabs = [{ id: 'tab-id', cwd: '/workspace', label: '#8', sessionTitle: 'pwd в файл', createdAt: new Date().toISOString() }]
  store().save(tabs, 'tab-id')
  assert.deepEqual(store().load(), { active: 'tab-id', tabs })
  const file = path.join(directory, 'workspace-sessions.json')
  const disk = fs.readFileSync(file, 'utf8')
  assert.equal(disk.includes('synthetic-token'), false)
  assert.equal(fs.existsSync(file + '.tmp'), false)
  const empty = { active: null, tabs: [] }
  for (const other of [store('/another'), store(undefined, 'ws://other'), store(undefined, undefined, 'rotated-token')]) {
    assert.deepEqual(other.load(), empty)
  }
  for (const bad of ['{', 'null', JSON.stringify({ ...JSON.parse(disk), active: 'unknown' }),
    JSON.stringify({ ...JSON.parse(disk), tabs: [tabs[0], tabs[0]] }),
    JSON.stringify({ ...JSON.parse(disk), tabs: [{ ...tabs[0], label: '#' + '9'.repeat(100) }] }),
    JSON.stringify({ ...JSON.parse(disk), tabs: [{ ...tabs[0], cwd: 42 }] })]) {
    fs.writeFileSync(file, bad)
    assert.deepEqual(store().load(), empty)
  }
  store().save([], null)
  assert.deepEqual(store().load(), empty)
})
