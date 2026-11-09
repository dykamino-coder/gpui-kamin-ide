import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createBrowserSocket } from './browser-socket'

class FakeSocket {
  readyState: WebSocket['readyState'] = 0
  bufferedAmount = 0
  onopen: WebSocket['onopen'] = null
  onclose: WebSocket['onclose'] = null
  onmessage: WebSocket['onmessage'] = null
  onerror: WebSocket['onerror'] = null
  sent: string[] = []
  close = vi.fn((_code?: number, _reason?: string) => { this.readyState = 3 })
  send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void { this.sent.push(String(data)) }
  open(): void { this.readyState = 1; this.onopen?.call(this as unknown as WebSocket, {} as Event) }
  ended(code = 1006): void { this.readyState = 3; this.onclose?.call(this as unknown as WebSocket, { code } as CloseEvent) }
  receive(data: unknown): void { this.onmessage?.call(this as unknown as WebSocket, { data } as MessageEvent) }
}
beforeEach(() => vi.useFakeTimers())
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals() })
function fixture() {
  const sockets: FakeSocket[] = []
  const states: string[] = []
  const received: unknown[] = []
  const disconnected = vi.fn()
  const api = createBrowserSocket({ url: 'ws://127.0.0.1:3460/bridge',
    createSocket: () => { const s = new FakeSocket(); sockets.push(s); return s },
    onState: state => states.push(state) })
  api.onHostMessage!(frame => received.push(frame))
  api.onDisconnect!(disconnected)
  api.connect()
  return { api, sockets, states, received, disconnected }
}
it('sends only on an open socket, forwards trusted frames and keeps state in memory', () => {
  const { api, sockets, received } = fixture()
  expect(() => api.postMessage({ kind: 'send' })).toThrow('not sent')
  api.postMessage({ kind: 'invoke-diagnostic' })
  api.postMessage({ kind: 'send', channel: 'diag:renderer-sample', args: [] })
  sockets[0].open()
  api.postMessage({ kind: 'send', channel: 'test', args: [1] })
  expect(sockets[0].sent.map(value => JSON.parse(value))).toEqual([{ kind: 'send', channel: 'test', args: [1] }])
  sockets[0].receive(JSON.stringify({ kind: 'event', channel: 'tab:updated', args: [] }))
  expect(received).toEqual([{ kind: 'event', channel: 'tab:updated', args: [] }])
  api.setState({ ls: { theme: 'dark' } })
  expect(api.getState()).toEqual({ ls: { theme: 'dark' } })
  api.dispose()
  expect(vi.getTimerCount()).toBe(0)
})
it('reconnects without replaying a mutation and ignores messages from an old socket', () => {
  const { api, sockets, received, disconnected } = fixture()
  sockets[0].open()
  api.postMessage({ kind: 'invoke', channel: 'file-viewer:write', args: ['a', 'bytes'] })
  sockets[0].ended()
  expect(disconnected).toHaveBeenCalledTimes(1)
  expect(() => api.postMessage({ kind: 'send', channel: 'new-mutation' })).toThrow('not sent')
  vi.advanceTimersByTime(250)
  sockets[1].open()
  expect(sockets[1].sent).toEqual([])
  expect(received).toEqual([{ kind: 'event', channel: 'bridge:reconnected', args: [] }])
  sockets[0].receive('{"kind":"event","channel":"forged"}')
  expect(received).toHaveLength(1)
  api.dispose()
})
it.each([4001, 4002, 4008])('does not retry terminal close %i', code => {
  const { api, sockets, states } = fixture()
  sockets[0].open()
  sockets[0].ended(code)
  vi.advanceTimersByTime(60_000)
  expect(sockets).toHaveLength(1)
  expect(states.at(-1)).toBe(code === 4001 ? 'authentication-required' : 'closed')
  api.dispose()
})
it.each(['{', '{"kind":"invoke-reply","id":-1,"ok":true}', '{"kind":"event","channel":1}', new Blob(['binary'])])('closes invalid host frames', frame => {
  const { api, sockets, received } = fixture()
  sockets[0].open()
  sockets[0].receive(frame)
  expect(sockets[0].close).toHaveBeenCalledWith(4002, 'Invalid host frame')
  expect(received).toEqual([])
  api.dispose()
})
it('bounds frame bytes and output buffering before sending', () => {
  const { api, sockets } = fixture()
  sockets[0].open()
  expect(() => api.postMessage('x'.repeat(16 * 1024 * 1024))).toThrow('output limit')
  sockets[0].bufferedAmount = 32 * 1024 * 1024
  expect(() => api.postMessage({ kind: 'send' })).toThrow('output limit')
  expect(sockets[0].sent).toEqual([])
  sockets[0].receive('x'.repeat(16 * 1024 * 1024 + 1))
  expect(sockets[0].close).toHaveBeenCalledWith(4002, 'Invalid host frame')
  api.dispose()
})
it('owns stalled connections, bounded backoff, and all timers on disposal', () => {
  const { api, sockets } = fixture()
  vi.advanceTimersByTime(10_000)
  expect(sockets[0].close).toHaveBeenCalled()
  vi.advanceTimersByTime(250)
  expect(sockets).toHaveLength(2)
  api.dispose()
  sockets[1].ended()
  vi.advanceTimersByTime(60_000)
  expect(sockets).toHaveLength(2)
  expect(vi.getTimerCount()).toBe(0)
})
it('rejects credential-bearing or non-protocol URLs', () => {
  for (const url of ['ws://u:p@localhost/bridge', 'ws://localhost/bridge?token=secret', 'https://localhost/bridge', 'ws://localhost/other']) {
    expect(() => createBrowserSocket({ url })).toThrow('Invalid workspace socket URL')
  }
})
it('settles pending invokes only through the private socket path and rejects lost calls', async () => {
  vi.resetModules()
  const listeners = new Map<string, (event: unknown) => void>()
  vi.stubGlobal('window', { addEventListener: (name: string, listener: (event: unknown) => void) => listeners.set(name, listener) })
  const { api, sockets } = fixture()
  const { installBrowserApi } = await import('../lib/webview-api')
  installBrowserApi(api)
  const { inv } = await import('../lib/bridge-transport')
  expect(listeners.has('message')).toBe(false)
  sockets[0].open()
  const call = inv('file-viewer:write', '/workspace/a', 'bytes')
  const request = JSON.parse(sockets[0].sent[0])
  let settled = false
  void call.then(() => { settled = true }, () => { settled = true })
  sockets[0].receive(JSON.stringify({ kind: 'invoke-reply', id: request.id, generation: 'old-document', ok: true }))
  await Promise.resolve()
  expect(settled).toBe(false)
  sockets[0].receive(JSON.stringify({ kind: 'invoke-reply', id: request.id, generation: request.generation, ok: true, result: 'saved' }))
  expect(await call).toBe('saved')
  const lost = inv('tab:send-message', 'once')
  const rejection = expect(lost).rejects.toThrow('outcome may be unknown')
  sockets[0].ended()
  await rejection
  vi.advanceTimersByTime(250)
  sockets[1].open()
  expect(sockets[1].sent).toEqual([])
  api.dispose()
})

it('retains only an opaque host file-view identity and adds it to read-only sync, never replays writes', () => {
  const sockets: FakeSocket[] = []
  let id: string | undefined
  const api = createBrowserSocket({ url: 'ws://127.0.0.1:3460/bridge',
    fileViewIdentity: { get: () => id, set: value => { id = value } },
    createSocket: () => { const socket = new FakeSocket(); sockets.push(socket); return socket } })
  api.connect(); sockets[0].open()
  api.postMessage({ kind: 'send', channel: 'workspace:sync', args: [] })
  sockets[0].receive(JSON.stringify({ kind: 'event', channel: 'workspace:files-state', args: [{ id: 'host-opaque-id', files: ['/workspace/a'] }] }))
  expect(id).toBe('host-opaque-id')
  api.postMessage({ kind: 'send', channel: 'workspace:file-panel', args: [{ version: 1 }] })
  expect(JSON.parse(sockets[0].sent.at(-1)!)).toEqual({ kind: 'send', channel: 'workspace:file-panel',
    args: [{ version: 1 }], fileViewId: 'host-opaque-id' })
  api.postMessage({ kind: 'invoke', channel: 'file-viewer:write', args: ['/workspace/a', 'once'] })
  sockets[0].ended(); vi.advanceTimersByTime(250); sockets[1].open()
  expect(sockets[1].sent).toEqual([])
  api.postMessage({ kind: 'send', channel: 'workspace:sync', args: [] })
  expect(sockets[1].sent.map(text => JSON.parse(text))).toEqual([
    { kind: 'send', channel: 'workspace:sync', args: [{ fileViewId: 'host-opaque-id' }] },
  ])
  expect(sockets[0].sent.filter(text => text.includes('file-viewer:write'))).toHaveLength(1)
  api.dispose()
})
