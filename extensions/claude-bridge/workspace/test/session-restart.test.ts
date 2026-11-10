import assert from 'node:assert/strict'
import { once, EventEmitter } from 'node:events'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import { createServer } from 'node:http'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { WebSocket, WebSocketServer } from 'ws'
import { startRuntimeProcess } from './fixtures/runtime-process.js'
import { waitFrame } from './fixtures/runtime-protocol.js'

test('real host restores open conversations, titles and active selection from disk after restart', async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-restart-'))
  const home = path.join(root, 'home'), tmp = path.join(root, 'tmp')
  fs.mkdirSync(home); fs.mkdirSync(tmp)
  const cleanup: Array<() => void | Promise<void>> = []
  t.after(async () => {
    for (const close of cleanup.reverse()) await close()
    assert.equal(path.dirname(root), fs.realpathSync(os.tmpdir()))
    assert.ok(path.basename(root).startsWith('bridgeweb-restart-'))
    fs.rmSync(root, { recursive: true })
  })
  const http = createServer((req, res) => { req.resume(); res.end('{"ok":true}') })
  const upstream = new WebSocketServer({ server: http })
  await new Promise<void>(resolve => http.listen(0, '127.0.0.1', resolve))
  const upstreamUrl = `ws://127.0.0.1:${(http.address() as { port: number }).port}`
  const sockets = new Map<string, WebSocket>()
  upstream.on('connection', socket => socket.on('message', raw => {
    const frame = JSON.parse(raw.toString())
    if (frame.type !== 'session:resume') return
    sockets.set(frame.conversationId, socket)
    socket.send(JSON.stringify({ type: 'session:created', sessionId: frame.conversationId }))
    socket.send(JSON.stringify({ type: 'session:conversation-id', conversationId: frame.conversationId }))
  }))
  cleanup.push(async () => {
    for (const socket of upstream.clients) socket.terminate()
    await new Promise<void>(resolve => upstream.close(() => resolve()))
    await new Promise<void>(resolve => http.close(() => resolve()))
  })
  async function boot() {
    const runtime = await startRuntimeProcess(root, tmp, home, upstreamUrl, cleanup)
    const login = await fetch(runtime.ready.origin + '/auth/login', { method: 'POST',
      headers: { Origin: runtime.ready.origin, 'Content-Type': 'application/json' }, body: JSON.stringify({ token: 'synthetic-workspace-token' }) })
    const socket = new WebSocket(runtime.ready.origin.replace('http:', 'ws:') + '/bridge',
      { headers: { Origin: runtime.ready.origin, Cookie: login.headers.get('set-cookie')!.split(';')[0] } })
    const frames: any[] = [], events = new EventEmitter()
    socket.on('message', raw => { frames.push(JSON.parse(raw.toString())); events.emit('frame') })
    await once(socket, 'open')
    cleanup.push(() => socket.terminate())
    let id = 0
    return { frames, events, send(channel: string, ...args: unknown[]) { socket.send(JSON.stringify({ kind: 'send', channel, args })) },
      async invoke(channel: string, ...args: unknown[]) {
        const requestId = ++id
        socket.send(JSON.stringify({ kind: 'invoke', id: requestId, generation: 'restart', channel, args }))
        const reply = await waitFrame(frames, events, f => f.kind === 'invoke-reply' && f.id === requestId)
        assert.equal(reply.ok, true, channel)
        return reply.result
      }, async stop(crash = false) {
        socket.close()
        if (!crash) runtime.child.send('shutdown')
        else if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(runtime.child.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true })
        else runtime.child.kill('SIGKILL')
        await runtime.exited
      } }
  }
  let view = await boot()
  const first = '00000000-0000-4000-8000-000000000001'
  const second = '00000000-0000-4000-8000-000000000002'
  const firstTab = await view.invoke('session:resume', { cwd: root, conversationId: first })
  await waitFrame(view.frames, view.events, f => f.channel === 'connection-state-changed' && f.args[0] === firstTab && f.args[1].status === 'connected' && f.args[1].sessionId === first)
  sockets.get(first)!.send(JSON.stringify({ type: 'session:title', title: 'Named persistent conversation' }))
  await waitFrame(view.frames, view.events, f => f.channel === 'tab:list-changed' && f.args[0].some((t: any) => t.sessionTitle === 'Named persistent conversation'))
  await view.invoke('tab:create', { cwd: root }) // Empty tabs have no conversation identity yet.
  const secondTab = await view.invoke('session:resume', { cwd: root, conversationId: second })
  view.send('tab:switch', firstTab)
  const before = await view.invoke('tab:list')
  assert.equal(await view.invoke('tab:get-active'), firstTab)
  await view.invoke('tabs:save-state', before.map((tab: any, i: number) => ({ id: tab.id, conversationId: tab.conversationId,
    title: tab.sessionTitle || tab.label, cwd: tab.cwd, pinned: i === 0, order: i, topOrder: 1 - i })))
  const disk = fs.readFileSync(path.join(root, 'storage', 'workspace-sessions.json'), 'utf8')
  assert.equal(disk.includes('synthetic-workspace-token'), false)
  await view.stop(true) // Persistence cannot depend on a graceful-shutdown hook.
  view = await boot()
  const restored = await view.invoke('tab:list')
  assert.equal(restored.length, 3, 'Host restart must recreate the open set before any browser asks')
  assert.deepEqual(restored.map((tab: any) => tab.id), before.map((tab: any) => tab.id), 'Pins/order must still match empty tabs')
  assert.deepEqual(restored.map((t: any) => [t.conversationId, t.sessionTitle, t.label, t.createdAt]),
    before.map((t: any) => [t.conversationId, t.sessionTitle, t.label, t.createdAt]))
  const active = restored.find((tab: any) => tab.conversationId === first)
  assert.equal(await view.invoke('tab:get-active'), active.id)
  assert.equal((await view.invoke('tabs:restore-state')).find((tab: any) => tab.conversationId === first).pinned, true)
  view.send('tab:close', active.id)
  await waitFrame(view.frames, view.events, f => f.channel === 'connection-state-changed' && f.args[0] === secondTab && f.args[1].status === 'connected' && f.args[1].sessionId === second)
  assert.equal(await view.invoke('tab:get-active'), secondTab, 'Closing the active restored tab must connect its lazy replacement')
  await view.invoke('tab:list') // Ordered dispatch barrier.
  await view.stop()
  view = await boot()
  assert.deepEqual((await view.invoke('tab:list')).map((tab: any) => tab.conversationId), [undefined, second], 'Closed conversations must not reopen')
  // History resume without an explicit label must not overwrite the saved name.
  const resumed = await view.invoke('session:resume', { cwd: root, conversationId: first })
  await waitFrame(view.frames, view.events, f => f.channel === 'connection-state-changed' && f.args[0] === resumed && f.args[1].status === 'connected' && f.args[1].sessionId === first)
  assert.equal((await view.invoke('tab:list')).find((tab: any) => tab.id === resumed).sessionTitle, 'Named persistent conversation')
  assert.equal((await view.invoke('session:get-saved')).find((s: any) => s.conversationId === first).label, 'Named persistent conversation')
  assert.equal((await view.invoke('tab:list')).find((tab: any) => tab.id === resumed).label, '#4', 'Restoration must advance per-folder numbering')
  assert.notEqual(firstTab, secondTab)
  view.send('disconnect')
  assert.deepEqual(await view.invoke('tab:list'), [])
  await view.stop()
  view = await boot()
  assert.deepEqual(await view.invoke('tab:list'), [], 'Explicit disconnect must durably clear the open set')
  await view.stop()
})
