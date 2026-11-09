import assert from 'node:assert/strict'
import { test } from 'node:test'
import { fixture, nodeCommand, until, alive } from './terminal-fixture.js'

test('PTY survives zero pages and restores output produced after the last detach', async t => {
  const f = await fixture(t), a = f.attach()
  const id = await a.spawn('chat-a')
  await a.send('pty:write', { ptyId: id, data: nodeCommand(
    "require('node:fs').writeFileSync('detached.pid',String(process.pid));setTimeout(()=>{console.log('zero-page-output');require('node:fs').writeFileSync('detached.done','yes')},400);setInterval(()=>{},1000)",
  ) })
  await until(async () => !!await f.read('detached.pid'))
  const pid = Number(await f.read('detached.pid'))
  a.view.dispose()
  await until(async () => !!await f.read('detached.done'), 'Closing the last page killed its shell')
  assert.ok(alive(pid))
  const b = f.attach()
  await b.send('workspace:terminal-sync', 'chat-a')
  const snapshot = (b.frames.find(f => f.channel === 'workspace:terminal-state')!.args as any[])[0]
  assert.equal(snapshot.ptyId, id)
  assert.match(snapshot.screen, /zero-page-output/)
  assert.equal(snapshot.exitCode, undefined)
  const ensured = await b.invoke('pty:spawn', { tabId: 'chat-a', cwd: f.root, cols: 80, rows: 24 })
  assert.equal((ensured.result as { ptyId: string }).ptyId, id)
  await b.send('pty:kill', { ptyId: id })
  await until(() => !alive(pid))
})


test('slow page detach preserves host output; snapshots are private and natural-exit screens persist', async t => {
  const f = await fixture(t), a = f.attach(), b = f.attach()
  const id = await a.spawn('retained-tab')
  a.rejectOutput()
  await b.send('pty:write', { ptyId: id, data: 'echo retained-output\r' })
  await until(() => f.disposed() === 1)
  await until(() => b.frames.some(f => f.channel === 'pty:data' && JSON.stringify(f.args).includes('retained-output')))
  const before = b.frames.filter(f => f.channel === 'workspace:terminal-state').length
  const c = f.attach()
  await c.send('workspace:terminal-sync', 'retained-tab')
  assert.equal(b.frames.filter(f => f.channel === 'workspace:terminal-state').length, before)
  const state = (c.frames.find(f => f.channel === 'workspace:terminal-state')!.args as any[])[0]
  assert.equal(state.ptyId, id)
  assert.match(state.screen, /retained-output/)
  await c.send('pty:write', { ptyId: id, data: 'exit 9\r' })
  await until(() => c.frames.some(f => f.channel === 'pty:exit'))
  c.view.dispose(); b.view.dispose()
  const d = f.attach()
  await d.send('workspace:terminal-sync', 'retained-tab')
  const exited = (d.frames.find(f => f.channel === 'workspace:terminal-state')!.args as any[])[0]
  assert.equal(exited.ptyId, id)
  assert.equal(exited.exitCode, 9)
  assert.match(exited.screen, /retained-output/)
  assert.ok(exited.sequence > state.sequence)
  await d.send('pty:kill', { ptyId: id })
  await d.send('workspace:terminal-sync', 'retained-tab')
  assert.equal((d.frames.filter(f => f.channel === 'workspace:terminal-state').at(-1)!.args as any[])[0], null)
})

test('closing one chat tab retains another tab terminal and cancels accepted pending spawns', async t => {
  const f = await fixture(t), a = f.attach()
  const first = await a.spawn('tab-a'), second = await a.spawn('tab-b')
  await a.send('tab:close', 'tab-a')
  await assert.rejects(a.send('pty:write', { ptyId: first, data: 'x' }), /Unknown/)
  await a.send('pty:write', { ptyId: second, data: 'echo unaffected-tab\r' })
  await until(() => a.frames.some(f => f.channel === 'pty:data' && JSON.stringify(f.args).includes('unaffected-tab')))
  const pending = a.invoke('pty:spawn', { tabId: 'pending-tab', cwd: f.root, cols: 80, rows: 24 })
  await a.send('tab:disconnect', 'pending-tab')
  assert.equal((await pending).ok, false)
  await a.send('workspace:terminal-sync', 'pending-tab')
  assert.equal((a.frames.filter(f => f.channel === 'workspace:terminal-state').at(-1)!.args as any[])[0], null)
})
