// Fault-injection contract with real WS sockets; production client owns retries.
import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { once } from 'node:events'
import { WebSocket, WebSocketServer } from 'ws'
import { startTransportProxy } from '../../../test-support/transport-proxy.mjs'

it.each([
  { type: 'mcp:response', result: 'synthetic success' },
  { type: 'mcp:response', result: 'Error: synthetic failure' },
  { type: 'mcp:denied', result: 'synthetic denial' },
])('cuts $result before delivery, then permits exactly one explicit retry', async ({ type, result }) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'fake-transport-'))
  const upstream = new WebSocketServer({ host: '127.0.0.1', port: 0 })
  await once(upstream, 'listening')
  const frames: unknown[] = []
  upstream.on('connection', (socket) => socket.on('message', (data) => frames.push(JSON.parse(data.toString()))))
  const proxy = await startTransportProxy({ root, targetPort: (upstream.address() as any).port, port: 0 })
  let client = new WebSocket(`ws://127.0.0.1:${proxy.port}/ws/session`)
  try {
    await once(client, 'open')
    client.send(JSON.stringify({ type: 'session:input', data: 'synthetic' }))
    await expect.poll(() => frames.length).toBe(1)
    fs.writeFileSync(path.join(root, 'cut-result'), '')
    const closed = once(client, 'close')
    client.send(JSON.stringify({ type, requestId: 'synthetic-id', result }))
    await closed
    expect(frames).toHaveLength(1)
    expect(fs.existsSync(path.join(root, 'result-cut'))).toBe(true)
    client = new WebSocket(`ws://127.0.0.1:${proxy.port}/ws/session`)
    await once(client, 'open')
    client.send(JSON.stringify({ type, requestId: 'synthetic-id', result }))
    await expect.poll(() => frames.length).toBe(2)
    expect(frames[1]).toMatchObject({ type, requestId: 'synthetic-id' })
  } finally {
    client.terminate()
    await proxy.close()
    for (const socket of upstream.clients) socket.terminate()
    await new Promise<void>((resolve) => upstream.close(() => resolve()))
    fs.rmSync(root, { recursive: true, force: true })
  }
})
describe('transport disconnection control', () => {
  it('disconnects live pairs using the external barrier without persisting messages', async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'fake-disconnect-'))
    const upstream = new WebSocketServer({ host: '127.0.0.1', port: 0 })
    await once(upstream, 'listening')
    const proxy = await startTransportProxy({ root, targetPort: (upstream.address() as any).port, port: 0 })
    const client = new WebSocket(`ws://127.0.0.1:${proxy.port}/ws/session`)
    try {
      await once(client, 'open')
      const closed = once(client, 'close')
      fs.writeFileSync(path.join(root, 'disconnect'), '')
      await closed
      await expect.poll(() => fs.existsSync(path.join(root, 'disconnected'))).toBe(true)
      expect(fs.readdirSync(root)).toEqual(['disconnected'])
    } finally {
      client.terminate()
      await proxy.close()
      for (const socket of upstream.clients) socket.terminate()
      await new Promise<void>((resolve) => upstream.close(() => resolve()))
      fs.rmSync(root, { recursive: true, force: true })
    }
  })
})
