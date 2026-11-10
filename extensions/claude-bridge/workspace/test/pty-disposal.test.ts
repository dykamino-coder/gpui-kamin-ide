import assert from 'node:assert/strict'
import { test } from 'node:test'
import { alive, fixture, until } from './terminal-fixture.js'

for (const mode of ['kill', 'tab:close', 'tab:disconnect', 'disconnect', 'shutdown'] as const) {
  test(`explicit ${mode} escalates an owned shell ignoring TERM and HUP`, {
    skip: process.platform !== 'linux' && 'Real POSIX signal regression runs in Linux Podman',
  }, async t => {
    const f = await fixture(t), a = f.attach()
    const reply = await a.invoke('pty:spawn', { tabId: 'resisting-tab', cwd: f.root, cols: 80, rows: 24, shellId: 'bash' })
    assert.equal(reply.ok, true, String(reply.error))
    const id = (reply.result as { ptyId: string }).ptyId
    await a.send('pty:write', { ptyId: id, data: `trap '' HUP TERM; printf '%s' "$$" > resisting.pid\r` })
    await until(async () => !!await f.read('resisting.pid'))
    const pid = Number(await f.read('resisting.pid'))
    assert.ok(alive(pid))

    // A second terminal must survive a targeted close; global shutdown owns both.
    const other = await a.spawn('unrelated-tab')
    await a.send('pty:write', { ptyId: other, data: `printf '%s' "$$" > unrelated.pid\r` })
    await until(async () => !!await f.read('unrelated.pid'))
    const otherPid = Number(await f.read('unrelated.pid'))
    assert.ok(alive(otherPid))
    if (mode === 'kill') await a.send('pty:kill', { ptyId: id })
    else if (mode === 'shutdown') f.backend.dispose()
    else await a.view.receive({ kind: 'send', channel: mode, args: mode === 'disconnect' ? [] : ['resisting-tab'] })
    // Duplicate disposal must neither reschedule cleanup nor affect other PTYs.
    if (mode === 'shutdown') f.backend.dispose()
    await until(() => !alive(pid), 'Shell ignoring TERM/HUP survived explicit teardown')
    if (mode === 'disconnect' || mode === 'shutdown') await until(() => !alive(otherPid))
    else {
      assert.ok(alive(otherPid), 'Targeted teardown terminated another terminal')
      await a.send('pty:kill', { ptyId: other })
      await until(() => !alive(otherPid))
    }
  })
}
