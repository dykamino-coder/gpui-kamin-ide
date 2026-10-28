// Local sockets only: exercise the actual request router, client pipeline and
// acknowledged outbox without launching a CLI, touching profiles or opening an IDE.
import { once } from 'node:events'
import { WebSocket, WebSocketServer } from 'ws'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { McpResultOutbox } from '../src/main/ws/mcp-result-outbox'
import { handleMcpCall, type McpCallCtx } from '../src/main/ws/mcp-call-pipeline'
import { pendingMcpCalls, rejectAllPending, sendMcpCall, settleMcpResult } from '../../server/src/core/pty/session-mcp-call'
import { sendToClient, WS_SEND_MAX_BUFFER } from '../../server/src/core/pty/session-io'
import type { PtySession } from '../../server/src/core/pty/types'

const execute = vi.hoisted(() => vi.fn())
vi.mock('../src/main/mcp/executor', () => ({ executeTool: execute }))
vi.mock('../../server/src/core/pty/session-input-coordinator', () => ({}))
vi.mock('../../server/src/core/logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn() }))

const cleanups: Array<() => Promise<void>> = []
afterEach(async () => {
  rejectAllPending('test cleanup')
  for (const cleanup of cleanups.splice(0)) await cleanup()
  vi.clearAllMocks()
})

async function loopback() {
  const listener = new WebSocketServer({ host: '127.0.0.1', port: 0 })
  await once(listener, 'listening')
  const address = listener.address()
  if (typeof address !== 'object' || !address) throw new Error('Missing loopback address')
  const port = address.port
  let client: WebSocket
  let server: WebSocket
  let dropAck = false
  let outcomes = 0
  const publish = vi.fn()
  const outbox = new McpResultOutbox(frame => {
    if (client.readyState !== WebSocket.OPEN || client.bufferedAmount + Buffer.byteLength(frame) > 4 * 1024 * 1024) return false
    client.send(frame)
    return true
  })
  const ctx = {
    window: { webContents: { send: publish } }, tabId: 'synthetic',
    pendingMcpCalls: new Map(), pendingPermissions: new Map(),
    permissionMode: () => 'bypassPermissions', permissionManager: {},
    deliverResult: (outcome: Parameters<McpResultOutbox['deliver']>[1]) => outbox.deliver('s', outcome),
    reportDeliveryFailure: vi.fn(), denyMcp: vi.fn(), send: vi.fn(),
  } as unknown as McpCallCtx
  const session = { id: 's', mcpCallCount: 0, inputCount: 0, lastActivityAt: new Date() } as PtySession

  async function connect() {
    const accepted = once(listener, 'connection')
    client = new WebSocket(`ws://127.0.0.1:${port}`)
    ;[server] = await accepted as [WebSocket]
    session.ws = server
    server.on('message', raw => {
      const outcome = JSON.parse(raw.toString())
      const accepted = settleMcpResult('s', outcome)
      outcomes++
      if (dropAck) { server.terminate(); return }
      server.send(JSON.stringify({ type: 'mcp:result-ack', requestId: outcome.requestId, accepted }))
    })
    client.on('message', raw => {
      const message = JSON.parse(raw.toString())
      if (message.type === 'mcp:call') void handleMcpCall(ctx, message)
      if (message.type === 'mcp:result-ack') outbox.acknowledge('s', message.requestId, message.accepted)
    })
    client.on('close', () => outbox.detach())
    await once(client, 'open')
    outbox.attach('s')
  }
  cleanups.push(async () => {
    outbox.dispose()
    client?.terminate()
    for (const socket of listener.clients) socket.terminate()
    await new Promise<void>(resolve => listener.close(() => resolve()))
  })
  await connect()
  return {
    session, ctx, publish, connect,
    get client() { return client }, get server() { return server },
    get outcomes() { return outcomes },
    set dropAck(value: boolean) { dropAck = value },
  }
}

describe('INC-2026-0080 local transport acceptance', () => {
  it('recovers a request after a genuinely paused socket drains, without reconnect', async () => {
    execute.mockResolvedValue({ content: [{ type: 'text', text: 'synthetic result' }] })
    const bridge = await loopback()
    bridge.client.pause()
    let frames = 0
    while (sendToClient(bridge.server, { type: 'synthetic-load', text: 'x'.repeat(128 * 1024) })) {
      frames++
      if (frames > 1000) throw new Error('Socket never reached its bounded send budget')
    }
    expect(bridge.server.bufferedAmount).toBeLessThanOrEqual(WS_SEND_MAX_BUFFER)
    const start = Date.now()
    const result = sendMcpCall(bridge.session, 'Read', { padding: 'x'.repeat(128 * 1024) })
    expect([...pendingMcpCalls.values()][0]?.delivered).toBe(false)
    bridge.client.resume()
    await expect(result).resolves.toMatchObject({ content: [{ text: 'synthetic result' }] })
    expect(Date.now() - start).toBeLessThan(3000)
    await vi.waitFor(() => expect(bridge.ctx.pendingMcpCalls.size).toBe(0))
    expect(execute).toHaveBeenCalledOnce()
  })

  it('returns a bounded correlated tool error for a large image result', async () => {
    execute.mockResolvedValue({ content: [{ type: 'image', mimeType: 'image/png', data: 'x'.repeat(6 * 1024 * 1024) }] })
    const bridge = await loopback()
    await expect(sendMcpCall(bridge.session, 'Read', {})).resolves.toMatchObject({ isError: true })
    await vi.waitFor(() => expect(bridge.ctx.pendingMcpCalls.size).toBe(0))
    expect(bridge.outcomes).toBe(1)
    expect(execute).toHaveBeenCalledOnce()
  })

  it('retains execution across reconnect and retransmits an outcome after a lost ack', async () => {
    let finish!: (value: unknown) => void
    execute.mockImplementation(() => new Promise(resolve => { finish = resolve }))
    const bridge = await loopback()
    const result = sendMcpCall(bridge.session, 'Write', {})
    await vi.waitFor(() => expect(execute).toHaveBeenCalledOnce())
    let closed = once(bridge.client, 'close')
    bridge.server.terminate()
    await closed
    finish('synthetic write finished while disconnected')
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(bridge.ctx.pendingMcpCalls.size).toBe(1)
    bridge.dropAck = true
    await bridge.connect()
    await expect(result).resolves.toBe('synthetic write finished while disconnected')
    if (bridge.client.readyState !== WebSocket.CLOSED) await once(bridge.client, 'close')
    bridge.dropAck = false
    await bridge.connect()
    await vi.waitFor(() => expect(bridge.ctx.pendingMcpCalls.size).toBe(0))
    expect(bridge.outcomes).toBe(2)
    expect(execute).toHaveBeenCalledOnce()
    expect(bridge.publish.mock.calls.filter(call => call[2]?.status === 'completed')).toHaveLength(1)
  })
})
