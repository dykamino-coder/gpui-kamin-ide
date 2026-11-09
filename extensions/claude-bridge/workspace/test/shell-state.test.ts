import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { applyShellOperation, defaultShellState, validateShellState } from '../../webview/shared/workspace-shell.js'
import { withWorkspaceShell } from '../src/shell-state.js'
import type { ClientFrame, HostFrame } from '../src/protocol.js'

test('dock moves keep every tool mounted in exactly one slot and select a valid fallback', () => {
  let state = defaultShellState()
  state = applyShellOperation(state, { type: 'move', panel: 'console', slot: 'main' })
  assert.deepEqual(state.panels.main, ['chat', 'console'])
  assert.equal(state.active.main, 'console')
  assert.equal(state.active.bottom, null)
  state = applyShellOperation(state, { type: 'move', panel: 'console', slot: 'right' })
  assert.equal(state.active.main, 'chat')
  assert.deepEqual(state.panels.right, ['agents', 'console'])
  state = applyShellOperation(state, { type: 'activate', panel: 'agents' })
  assert.equal(state.active.right, 'agents')
  assert.equal(state.revision, 3)
  assert.equal(defaultShellState().revision, 0)
  for (const op of [{ type: 'resize', mainRatio: NaN }, { type: 'resize', sidebarWidth: 199 },
    { type: 'move', panel: 'files', slot: 'main' }, { type: 'move', panel: 'chat', slot: 'other' },
    { type: 'sidebar', visible: 1 }, { type: 'project', cwd: '/workspace', expanded: 'yes' }, { type: 'unknown' }]) {
    assert.throws(() => applyShellOperation(state, op))
  }
  assert.throws(() => validateShellState({ ...state, panels: { ...state.panels, bottom: ['chat'] } }))
})

test('host serializes concurrent view operations, broadcasts commits and restores after zero views/restart', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-shell-'))
  t.after(() => { assert.equal(fs.realpathSync(path.dirname(directory)), fs.realpathSync(os.tmpdir())); fs.rmSync(directory, { recursive: true }) })
  const file = path.join(directory, 'state.json')
  const forwarded: ClientFrame[] = []
  const inner = { attach() { return { async receive(frame: ClientFrame) { forwarded.push(frame) }, dispose() {} } } }
  const backend = withWorkspaceShell(inner, file)
  const a: HostFrame[] = [], b: HostFrame[] = []
  const first = backend.attach({ async postMessage(frame) { a.push(frame); return true } })
  const second = backend.attach({ async postMessage(frame) { b.push(frame); return true } })
  const request = (id: number, operation: unknown): ClientFrame => ({ kind: 'invoke', id, generation: 'test', channel: 'workspace:shell:update', args: [operation] })
  await Promise.all([
    first.receive(request(1, { type: 'sidebar', visible: false })),
    second.receive(request(2, { type: 'move', panel: 'agents', slot: 'bottom' })),
  ])
  for (const frames of [a, b]) assert.deepEqual(frames.filter(f => f.kind === 'event').map(f => (f.args as any)[0].revision), [1, 2])
  first.dispose(); second.dispose()
  const restored = withWorkspaceShell(inner, file)
  const frames: HostFrame[] = []
  const reopened = restored.attach({ async postMessage(frame) { frames.push(frame); return true } })
  await reopened.receive({ kind: 'send', channel: 'workspace:sync', args: [] })
  const snapshot = (frames[0].args as any)[0]
  assert.equal(snapshot.sidebarVisible, false)
  assert.equal(snapshot.revision, 2)
  assert.deepEqual(snapshot.panels.bottom, ['console', 'agents'])
  assert.deepEqual(forwarded.map(f => f.channel), ['workspace:sync'])
  await reopened.receive(request(3, { type: 'move', panel: 'chat', slot: 'invalid' }))
  assert.equal(frames.at(-1)?.ok, false)
  assert.equal(JSON.parse(fs.readFileSync(file, 'utf8')).revision, 2)
  reopened.dispose()
})

test('failed persistence never acknowledges or broadcasts an uncommitted layout', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-shell-'))
  t.after(() => { assert.equal(fs.realpathSync(path.dirname(directory)), fs.realpathSync(os.tmpdir())); fs.rmSync(directory, { recursive: true }) })
  const blocker = path.join(directory, 'not-a-directory')
  fs.writeFileSync(blocker, '')
  const backend = withWorkspaceShell({ attach() { return { async receive() {}, dispose() {} } } }, path.join(blocker, 'layout.json'))
  const frames: HostFrame[] = []
  const client = backend.attach({ async postMessage(frame) { frames.push(frame); return true } })
  await client.receive({ kind: 'invoke', id: 1, generation: 'test', channel: 'workspace:shell:update', args: [{ type: 'sidebar', visible: false }] })
  assert.equal(frames.length, 1)
  assert.equal(frames[0].ok, false)
  await client.receive({ kind: 'invoke', id: 2, generation: 'test', channel: 'workspace:shell:get', args: [] })
  assert.equal((frames[1].result as any).sidebarVisible, true)
  client.dispose()
})
