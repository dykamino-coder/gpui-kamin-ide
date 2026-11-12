import { PassThrough } from 'node:stream'
import { describe, expect, it, vi } from 'vitest'
import { JsonRpcClosedError, JsonRpcConnection, JsonRpcError, JsonRpcTimeoutError, METHOD_NOT_FOUND } from './jsonrpc'

/** Peer side: what Bridge wrote, and a way to answer as codex would. */
function pair(options?: ConstructorParameters<typeof JsonRpcConnection>[2]) {
  const fromPeer = new PassThrough()
  const toPeer = new PassThrough()
  const sent: Array<Record<string, unknown>> = []
  let partial = ''
  toPeer.on('data', (chunk: Buffer) => {
    partial += chunk.toString('utf8')
    let i: number
    while ((i = partial.indexOf('\n')) >= 0) {
      sent.push(JSON.parse(partial.slice(0, i)))
      partial = partial.slice(i + 1)
    }
  })
  const conn = new JsonRpcConnection(fromPeer, toPeer, options)
  const reply = (message: unknown) => fromPeer.write(JSON.stringify(message) + '\n')
  return { conn, sent, reply, fromPeer }
}

describe('JsonRpcConnection', () => {
  it('frames requests without a jsonrpc member and resolves by id', async () => {
    const { conn, sent, reply } = pair()
    const result = conn.request('initialize', { clientInfo: { name: 'x' } })
    await vi.waitFor(() => expect(sent).toHaveLength(1))
    expect(sent[0]).toEqual({ id: 1, method: 'initialize', params: { clientInfo: { name: 'x' } } })
    reply({ id: 1, result: { userAgent: 'codex' } })
    await expect(result).resolves.toEqual({ userAgent: 'codex' })
  })

  it('maps error responses to JsonRpcError with the peer code', async () => {
    const { conn, reply } = pair()
    const result = conn.request('turn/steer', {})
    reply({ id: 1, error: { code: -32600, message: 'no active turn', data: { k: 1 } } })
    await expect(result).rejects.toMatchObject({ name: 'JsonRpcError', code: -32600, message: 'no active turn' })
  })

  it('reassembles messages split across chunks, including multibyte characters', async () => {
    const { conn, fromPeer } = pair()
    const seen: unknown[] = []
    conn.onNotification((method, params) => seen.push({ method, params }))
    const line = Buffer.from(JSON.stringify({ method: 'item/agentMessage/delta', params: { delta: 'привет' } }) + '\n')
    for (let i = 0; i < line.length; i += 3) fromPeer.write(line.subarray(i, i + 3))
    await vi.waitFor(() => expect(seen).toEqual([{ method: 'item/agentMessage/delta', params: { delta: 'привет' } }]))
  })

  it('times out a request and ignores its late response as a protocol error', async () => {
    const errors: string[] = []
    const { conn, reply } = pair({ onProtocolError: (e) => errors.push(e.message) })
    await expect(conn.request('model/list', {}, { timeoutMs: 20 })).rejects.toBeInstanceOf(JsonRpcTimeoutError)
    reply({ id: 1, result: {} })
    await vi.waitFor(() => expect(errors).toEqual(['response for unknown request id 1']))
    expect(conn.closed).toBe(false)
  })

  it('honours an abort signal', async () => {
    const { conn } = pair()
    const abort = new AbortController()
    const result = conn.request('thread/start', {}, { signal: abort.signal })
    abort.abort(new Error('caller gone'))
    await expect(result).rejects.toThrow('caller gone')
  })

  it('answers server requests through the handler and reports handler errors', async () => {
    const { conn, sent, reply } = pair()
    conn.setServerRequestHandler((method) => {
      if (method === 'item/commandExecution/requestApproval') return { decision: 'decline' }
      throw new JsonRpcError(-32001, 'refused')
    })
    reply({ id: 7, method: 'item/commandExecution/requestApproval', params: {} })
    reply({ id: 8, method: 'item/tool/call', params: {} })
    await vi.waitFor(() => expect(sent).toHaveLength(2))
    expect(sent).toContainEqual({ id: 7, result: { decision: 'decline' } })
    expect(sent).toContainEqual({ id: 8, error: { code: -32001, message: 'refused' } })
  })

  it('answers method-not-found when no handler is installed', async () => {
    const { sent, reply } = pair()
    reply({ id: 'srv-1', method: 'currentTime/read', params: {} })
    await vi.waitFor(() => expect(sent).toHaveLength(1))
    expect(sent[0]).toMatchObject({ id: 'srv-1', error: { code: METHOD_NOT_FOUND } })
  })

  it('reports garbage lines without closing', async () => {
    const errors: string[] = []
    const { conn, fromPeer, reply } = pair({ onProtocolError: (e) => errors.push(e.message) })
    fromPeer.write('WARNING: not json\n[1,2]\n')
    const result = conn.request('x')
    reply({ id: 1, result: 'ok' })
    await expect(result).resolves.toBe('ok')
    expect(errors).toHaveLength(2)
  })

  it('rejects every pending request when the peer closes its output', async () => {
    const { conn, fromPeer } = pair()
    const closed = vi.fn()
    conn.onClose(closed)
    const a = conn.request('a')
    const b = conn.request('b')
    fromPeer.end()
    await expect(a).rejects.toBeInstanceOf(JsonRpcClosedError)
    await expect(b).rejects.toBeInstanceOf(JsonRpcClosedError)
    expect(closed).toHaveBeenCalledTimes(1)
    await expect(conn.request('c')).rejects.toBeInstanceOf(JsonRpcClosedError)
    expect(() => conn.notify('initialized')).toThrow(JsonRpcClosedError)
  })

  it('closes when a line exceeds the limit', async () => {
    const { conn, fromPeer } = pair({ maxLineBytes: 16 })
    const pending = conn.request('a')
    fromPeer.write('x'.repeat(32))
    await expect(pending).rejects.toThrow('line exceeds 16 bytes')
    expect(conn.closed).toBe(true)
  })

  it('close is idempotent and keeps the first reason', async () => {
    const { conn } = pair()
    conn.close(new JsonRpcClosedError('first'))
    conn.close(new JsonRpcClosedError('second'))
    await expect(conn.request('x')).rejects.toThrow('first')
    const late = vi.fn()
    conn.onClose(late)
    expect(late).toHaveBeenCalledWith(expect.objectContaining({ message: expect.stringContaining('first') }))
  })
})
