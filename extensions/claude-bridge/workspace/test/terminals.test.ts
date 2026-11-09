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
  await assert.rejects(a.send('pty:write', { ptyId: id, data: 'echo stale\r' }), /not owned/)
  assert.ok(a.frames.filter(frame => frame.kind === 'invoke-reply').every(frame => frame.generation === 'terminal-fixture'))
})

for (const mode of ['kill', 'disconnect', 'shutdown', 'backpressure'] as const) {
  test(`terminal ${mode} terminates the owned long-running child without closing another view`, async t => {
    const f = await fixture(t)
    const a = f.attach(), b = f.attach()
    const id = await a.spawn()
    await assert.rejects(b.send('pty:kill', { ptyId: id }), /not owned/)
    await a.send('pty:write', { ptyId: id, data: nodeCommand(
      "require('node:fs').writeFileSync('child.pid',String(process.pid));setInterval(()=>console.log('alive'),100)",
    ) })
    await until(async () => !!await f.read('child.pid'))
    const pid = Number(await f.read('child.pid'))
    assert.ok(alive(pid))
    // Clean up only this fixture's known PID even if the regression assertion fails.
    t.after(async () => {
      if (alive(pid)) {
        if (process.platform === 'win32') {
          const { execFile } = await import('node:child_process')
          await new Promise(resolve => execFile('taskkill', ['/T', '/F', '/PID', String(pid)], { windowsHide: true }, resolve))
        } else process.kill(pid, 'SIGKILL')
      }
    })
    if (mode === 'kill') await a.send('pty:kill', { ptyId: id })
    if (mode === 'disconnect') a.view.dispose()
    if (mode === 'shutdown') f.backend.dispose()
    if (mode === 'backpressure') a.rejectOutput()
    await until(() => !alive(pid), 'PTY child survived teardown')
    if (mode === 'kill') await until(() => a.frames.some(frame => frame.channel === 'pty:exit'))
    if (mode === 'disconnect' || mode === 'backpressure') {
      assert.equal((await b.invoke('shells:list')).ok, true)
      assert.equal(f.disposed(), 1)
    }
    a.view.dispose(); b.view.dispose(); f.backend.dispose()
    assert.equal(f.disposed(), 2)
  })
}

test('PTY validates input, ownership, profiles and cwd; unknown channels still reach shared backend', async t => {
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

test('concurrent spawn limits count pending work; disposed views never leak a late PTY', async t => {
  const f = await fixture(t), a = f.attach()
  await Promise.all(Array.from({ length: 5 }, () => a.invoke('pty:spawn', { cwd: f.root, cols: 80, rows: 24 })))
  assert.equal(a.frames.filter(frame => frame.kind === 'invoke-reply' && frame.ok).length, 4)
  assert.match(String(a.frames.find(frame => frame.ok === false)?.error), /limit/)
  a.view.dispose()
  const b = f.attach()
  const pending = b.invoke('pty:spawn', { cwd: f.root, cols: 80, rows: 24 })
  b.view.dispose()
  await pending
  assert.equal(b.frames.filter(frame => frame.ok).length, 0)
  assert.throws(() => { f.backend.dispose(); f.attach() }, /disposed/)
})
