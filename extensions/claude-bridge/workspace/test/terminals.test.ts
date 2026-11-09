import assert from 'node:assert/strict'
import { test } from 'node:test'
import { alive, fixture, nodeCommand, until } from './terminal-fixture.js'

test('shared terminal executes in cwd with TTY size; data, resize and natural exit match bridge frames', async t => {
  const f = await fixture(t)
  const a = f.attach()
  const listed = await a.invoke('shells:list')
  assert.equal(listed.ok, true)
  assert.ok((listed.result as unknown[]).length)
  const id = await a.spawn()
  await a.send('pty:resize', { ptyId: id, cols: 93, rows: 31 })
  await a.send('pty:write', { ptyId: id, data: nodeCommand(
    "require('node:fs').writeFileSync('tty.json',JSON.stringify({cwd:process.cwd(),cols:process.stdout.columns,rows:process.stdout.rows}));console.log('terminal-observed')",
  ) })
  await until(async () => !!await f.read('tty.json'))
  assert.deepEqual(JSON.parse(await f.read('tty.json')), { cwd: f.root, cols: 93, rows: 31 })
  await until(() => a.frames.some(frame => frame.channel === 'pty:data' && JSON.stringify(frame.args).includes('terminal-observed')))
  await a.send('pty:write', { ptyId: id, data: 'exit 7\r' })
  await until(() => a.frames.some(frame => frame.channel === 'pty:exit'))
  const exited = a.frames.find(frame => frame.channel === 'pty:exit')!
  assert.equal((exited.args as { exitCode: number }[])[0].exitCode, 7)
  await assert.rejects(a.send('pty:write', { ptyId: id, data: 'echo stale\r' }), /has exited/)
  assert.ok(a.frames.filter(frame => frame.kind === 'invoke-reply').every(frame => frame.generation === 'terminal-fixture'))
})

for (const mode of ['kill', 'tab:close', 'tab:disconnect', 'disconnect', 'shutdown'] as const) {
  test(`explicit ${mode} terminates the owned long-running child`, async t => {
    const f = await fixture(t), a = f.attach(), b = f.attach()
    const id = await a.spawn('target-tab')
    await a.send('pty:write', { ptyId: id, data: nodeCommand(
      "require('node:fs').writeFileSync('child.pid',String(process.pid));setInterval(()=>console.log('alive'),100)",
    ) })
    await until(async () => !!await f.read('child.pid'))
    const pid = Number(await f.read('child.pid'))
    assert.ok(alive(pid))
    if (mode === 'kill') await b.send('pty:kill', { ptyId: id })
    else if (mode === 'shutdown') f.backend.dispose()
    else await b.view.receive({ kind: 'send', channel: mode, args: mode === 'disconnect' ? [] : ['target-tab'] })
    await until(() => !alive(pid), 'PTY child survived explicit teardown')
    if (mode !== 'shutdown') assert.equal((await b.invoke('shells:list')).ok, true)
    a.view.dispose(); b.view.dispose(); f.backend.dispose()
    assert.equal(f.disposed(), 2)
  })
}

test('PTY validates input, profiles and cwd; unknown channels still reach shared backend', async t => {
  const f = await fixture(t), a = f.attach()
  for (const patch of [{ cols: 0 }, { rows: 1001 }, { shellId: 'does-not-exist' }, { cwd: 'relative' }, { cwd: f.root + '/missing' }]) {
    const reply = await a.invoke('pty:spawn', { cwd: f.root, cols: 80, rows: 24, ...patch })
    assert.equal(reply.ok, false)
    assert.equal(typeof reply.error, 'string')
  }
  assert.equal((await a.invoke('shells:list', 1)).ok, false)
  const id = await a.spawn()
  await assert.rejects(a.send('pty:resize', { ptyId: id, cols: 2.1, rows: 3 }), /dimensions/)
  await assert.rejects(a.send('pty:write', { ptyId: id, data: 'é'.repeat(40000) }), /limit/)
  await a.view.receive({ kind: 'send', channel: 'fixture:next', args: [] })
  assert.deepEqual(f.delegated, [{ kind: 'send', channel: 'fixture:next', args: [] }])
})

test('pending spawns coalesce per tab, survive lost replies and obey the host screen limit', async t => {
  const f = await fixture(t), a = f.attach()
  const opts = { tabId: 'shared-tab', cwd: f.root, cols: 80, rows: 24 }
  const replies = await Promise.all(Array.from({ length: 5 }, () => a.invoke('pty:spawn', opts)))
  assert.ok(replies.every(r => r.ok))
  assert.equal(new Set(replies.map(r => (r.result as { ptyId: string }).ptyId)).size, 1)
  const large = await Promise.all(Array.from({ length: 4 }, (_, i) => a.invoke('pty:spawn', {
    tabId: `large-${i}`, cwd: f.root, cols: 500, rows: 200,
  })))
  assert.equal(large.filter(r => r.ok).length, 2)
  assert.match(String(large.find(r => !r.ok)?.error), /resource limit/)
  const b = f.attach()
  const pending = b.invoke('pty:spawn', { tabId: 'lost-reply', cwd: f.root, cols: 80, rows: 24 })
  b.view.dispose()
  await pending
  assert.equal(b.frames.filter(frame => frame.ok).length, 0)
  const c = f.attach()
  await c.send('workspace:terminal-sync', 'lost-reply')
  assert.ok((c.frames.find(f => f.channel === 'workspace:terminal-state')!.args as any[])[0].ptyId)
  assert.throws(() => { f.backend.dispose(); f.attach() }, /disposed/)
})
