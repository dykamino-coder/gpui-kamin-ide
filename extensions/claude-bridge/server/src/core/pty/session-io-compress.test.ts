import { afterEach, expect, it } from 'vitest'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { WebSocket, WebSocketServer } from 'ws'
import { trySendToClient } from './session-io'
import { DEFAULT_WS_COMPRESS_MIN_BYTES, wsCompressMinBytes, wsDeflateOptions } from './ws-compression'

const closers: Array<() => void> = []
afterEach(() => {
  for (const c of closers.splice(0)) c()
})

it('reads the compression cut-off from the environment', () => {
  expect(wsCompressMinBytes({})).toBe(DEFAULT_WS_COMPRESS_MIN_BYTES)
  expect(wsCompressMinBytes({ BRIDGE_WS_COMPRESS_MIN_BYTES: '0' })).toBe(0)
  expect(wsCompressMinBytes({ BRIDGE_WS_COMPRESS_MIN_BYTES: '4096' })).toBe(4096)
  expect(wsCompressMinBytes({ BRIDGE_WS_COMPRESS_MIN_BYTES: 'lots' })).toBe(DEFAULT_WS_COMPRESS_MIN_BYTES)
})

it('sends small live frames raw and bulk frames deflated on one permessage-deflate connection', async () => {
  const http: Server = createServer()
  const wss = new WebSocketServer({ server: http, perMessageDeflate: wsDeflateOptions({}) })
  closers.push(() => {
    wss.close()
    http.close()
  })
  await new Promise<void>((r) => http.listen(0, '127.0.0.1', r))
  const port = (http.address() as AddressInfo).port
  const serverSide = new Promise<import('ws').WebSocket>((r) => wss.once('connection', r))
  // The `ws` client offers permessage-deflate by default, like the KaminIDE host.
  const client = new WebSocket(`ws://127.0.0.1:${port}`)
  closers.push(() => client.terminate())
  const got: string[] = []
  client.on('message', (m) => got.push(m.toString()))
  await new Promise((r) => client.once('open', r))
  const ws = await serverSide
  expect(client.extensions).toContain('permessage-deflate')

  const record = { type: 'assistant', uuid: 'u', message: { content: [{ type: 'text', text: 'live '.repeat(300) }] } }
  const small = { type: 'jsonl:entries', entries: [record] }
  const bulk = { type: 'jsonl:entries', replay: true, entries: Array.from({ length: 40 }, () => record) }
  const sock = (client as unknown as { _socket: { bytesRead: number } })._socket
  const wire = async (msg: Record<string, unknown>): Promise<number> => {
    const before = sock.bytesRead
    const n = got.length
    expect(trySendToClient(ws as never, msg)).toBe('sent')
    await expect.poll(() => got.length).toBe(n + 1)
    return sock.bytesRead - before
  }
  const smallBytes = await wire(small)
  const bulkBytes = await wire(bulk)
  const smallRaw = Buffer.byteLength(JSON.stringify(small))
  const bulkRaw = Buffer.byteLength(JSON.stringify(bulk))
  expect(smallRaw).toBeLessThan(DEFAULT_WS_COMPRESS_MIN_BYTES)
  expect(smallBytes).toBeGreaterThanOrEqual(smallRaw) // raw frame plus header
  expect(bulkBytes).toBeLessThan(bulkRaw / 5) // deflated
  expect(got.map((m) => JSON.parse(m))).toEqual([small, bulk])
})
