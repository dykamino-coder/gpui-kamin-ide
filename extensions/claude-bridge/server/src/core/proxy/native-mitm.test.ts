// Exercise the production request handler without opening host listeners or TLS sockets.
import { Buffer } from 'node:buffer'
import { EventEmitter } from 'node:events'
import type { IncomingMessage, ServerResponse } from 'node:http'
import { Readable, Writable } from 'node:stream'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

// Fresh handler imports can be slow on a busy Windows runner. Keep asynchronous
// lifecycle assertions within this file's deadline, rather than leaking work
// into the next test when Vitest's default one-second wait expires.
vi.setConfig({ testTimeout: 30_000 })
const waitFor = (assertion: () => void) => vi.waitFor(assertion, { timeout: 15_000 })

const mocks = vi.hoisted(() => ({
  fetch: vi.fn(),
  build: vi.fn(),
  log: vi.fn(),
  servers: [] as EventEmitter[],
  ca: '',
}))
vi.mock('undici', () => ({ fetch: mocks.fetch }))
vi.mock('./outbound-tls', () => ({ createOutboundDispatcher: mocks.build }))
vi.mock('../config/settings', () => ({ getSetting: () => mocks.ca }))
vi.mock('../logging', () => ({ warnLog: mocks.log }))
vi.mock('./ca-store', () => ({ getOrCreateBridgeCA: async () => ({ certPath: 'synthetic-ca' }) }))
vi.mock('./leaf-cert', () => ({ mintLeafCert: vi.fn() }))
vi.mock('node:https', () => ({
  createServer: () => {
    const server = new EventEmitter()
    mocks.servers.push(server)
    return server
  },
}))
vi.mock('node:net', () => ({
  createServer: () =>
    Object.assign(new EventEmitter(), {
      listen: (_port: number, _host: string, done: () => void) => done(),
      address: () => ({ port: 12345 }),
    }),
}))

class CliResponse extends Writable {
  statusCode = 200
  headersSent = false
  setHeader = vi.fn()
  chunks: Buffer[] = []
  pending?: () => void
  constructor(public blocked = false) {
    super({ highWaterMark: 16_384 })
    this.on('error', () => {})
  }
  override _write(chunk: Buffer, _encoding: BufferEncoding, done: () => void) {
    this.chunks.push(chunk)
    if (this.blocked) this.pending = done
    else done()
  }
  unblock() {
    this.blocked = false
    this.pending?.()
    this.pending = undefined
  }
}

const tick = () => new Promise<void>((resolve) => setImmediate(resolve))
const responses: CliResponse[] = []
function request(server = mocks.servers.at(-1)!, blocked = false): CliResponse {
  const req = Object.assign(Readable.from([]), {
    headers: { host: 'api.anthropic.com' },
    method: 'POST',
    url: '/v1/messages',
  })
  const res = new CliResponse(blocked)
  responses.push(res)
  server.emit('request', req as IncomingMessage, res as unknown as ServerResponse)
  return res
}
function resetError() {
  return new TypeError('fetch failed', {
    cause: Object.assign(new Error('other side closed'), { code: 'UND_ERR_SOCKET' }),
  })
}

beforeEach(() => {
  vi.resetModules()
  vi.clearAllMocks()
  mocks.servers.length = 0
  mocks.ca = ''
  mocks.build.mockImplementation(() => ({ close: vi.fn(async () => {}) }))
})
afterEach(async () => {
  for (const res of responses.splice(0)) res.destroy()
  await tick()
})

it('late-old-generation-error cannot close the replacement shared pool', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  const opts = { ptySessionId: 'synthetic', onEntryUpdate: vi.fn() }
  await startNativeMitm(opts)
  const failures: ((error: Error) => void)[] = []
  mocks.fetch.mockImplementation(() => new Promise((_resolve, reject) => failures.push(reject)))
  request()
  await waitFor(() => expect(failures).toHaveLength(1))
  const lateResponse = request()
  await waitFor(() => expect(failures).toHaveLength(2))
  failures[0]!(resetError())
  const first = mocks.build.mock.results[0]!.value
  await waitFor(() => expect(first.close).toHaveBeenCalledTimes(1))
  await startNativeMitm({ ...opts, ptySessionId: 'neighbor' })
  const replacement = mocks.build.mock.results[1]!.value
  request()
  await waitFor(() => expect(failures).toHaveLength(3))
  failures[1]!(resetError())
  await waitFor(() => expect(lateResponse.writableFinished).toBe(true))
  expect(replacement.close).not.toHaveBeenCalled()
  request()
  await waitFor(() => expect(failures).toHaveLength(4))
  expect(mocks.build).toHaveBeenCalledTimes(2)
  expect(mocks.fetch.mock.calls.at(-1)![1].dispatcher).toBe(replacement)
  expect(mocks.log).toHaveBeenCalledWith(
    '[mitm][pool] stale reset ignored',
    expect.objectContaining({
      generation: 1,
      currentGeneration: 2,
      dispatcherResets: 1,
      staleDispatcherResets: 1,
    }),
  )
})

it('mitm-no-drain pauses upstream instead of buffering a 20 MiB SSE response', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  await startNativeMitm({ ptySessionId: 'blocked', onEntryUpdate: vi.fn() })
  let pulls = 0
  const chunk = Buffer.alloc(64 * 1024, 32)
  chunk[0] = 58 // an artificial SSE keepalive comment
  chunk.write('\n\n', chunk.length - 2)
  const body = new ReadableStream({
    pull(controller) {
      if (pulls++ < 320) controller.enqueue(chunk)
      else controller.close()
    },
  })
  mocks.fetch.mockResolvedValue({ status: 200, headers: new Headers({ 'content-type': 'text/event-stream' }), body })
  const res = request(undefined, true)
  await waitFor(() => expect(res.writableLength).toBeGreaterThan(0))
  for (let i = 0; i < 10; i++) await tick()
  expect(res.writableLength).toBeLessThanOrEqual(256 * 1024)
  expect(pulls).toBeLessThan(320)
  expect(mocks.fetch).toHaveBeenCalledTimes(1)
})

it('serializes concurrent configuration swaps without waiting for old pool drain', async () => {
  const { startNativeMitm, resetExtraTrustRoots } = await import('./native-mitm')
  const opts = { ptySessionId: 'config', onEntryUpdate: vi.fn() }
  await startNativeMitm(opts)
  const first = mocks.build.mock.results[0]!.value
  first.close.mockImplementation(() => new Promise(() => {}))
  await Promise.all(
    Array.from({ length: 10 }, () => startNativeMitm({ ...opts, upstreamProxyUrl: 'http://proxy.example' })),
  )
  expect(mocks.build).toHaveBeenCalledTimes(2)
  expect(first.close).toHaveBeenCalledTimes(1)
  resetExtraTrustRoots()
  mocks.ca = 'synthetic-next-ca.pem'
  await Promise.all([startNativeMitm(opts), startNativeMitm(opts)])
  expect(mocks.build.mock.calls).toEqual([
    ['', undefined],
    ['', 'http://proxy.example'],
    ['synthetic-next-ca.pem', undefined],
  ])
  expect(mocks.build).toHaveBeenLastCalledWith('synthetic-next-ca.pem', undefined)
})

function streamResponse(body: ReadableStream<Uint8Array>, contentType = 'text/event-stream') {
  return { status: 200, headers: new Headers({ 'content-type': contentType }), body }
}

it.each(['text/event-stream', 'application/json'])(
  'ignores a late old %s stream reset after CA replacement',
  async (contentType) => {
    const { startNativeMitm, resetExtraTrustRoots } = await import('./native-mitm')
    const opts = { ptySessionId: 'old-stream', onEntryUpdate: vi.fn() }
    await startNativeMitm(opts)
    let controller!: ReadableStreamDefaultController<Uint8Array>
    mocks.fetch.mockResolvedValue(
      streamResponse(
        new ReadableStream({
          start(c) {
            controller = c
          },
        }),
        contentType,
      ),
    )
    const res = request()
    await waitFor(() => expect(res.listenerCount('error')).toBeGreaterThan(1))
    resetExtraTrustRoots()
    await startNativeMitm({ ...opts, ptySessionId: 'new-stream' })
    const replacement = mocks.build.mock.results[1]!.value
    controller.error(Object.assign(new Error('old reset'), { code: 'EPIPE' }))
    await waitFor(() => expect(res.destroyed).toBe(true))
    expect(replacement.close).not.toHaveBeenCalled()
    expect(mocks.fetch).toHaveBeenCalledTimes(1)
    await startNativeMitm(opts)
    expect(mocks.build).toHaveBeenCalledTimes(2)
  },
)

it.each(['text/event-stream', 'application/json'])(
  'recycles CURRENT pool on %s stream reset without retry',
  async (contentType) => {
    const { startNativeMitm } = await import('./native-mitm')
    const opts = { ptySessionId: 'reset', onEntryUpdate: vi.fn() }
    await startNativeMitm(opts)
    let controller!: ReadableStreamDefaultController<Uint8Array>
    mocks.fetch.mockResolvedValue(
      streamResponse(
        new ReadableStream({
          start(c) {
            controller = c
          },
        }),
        contentType,
      ),
    )
    const res = request()
    await waitFor(() => expect(res.listenerCount('error')).toBeGreaterThan(1))
    controller.error(Object.assign(new Error('reset'), { code: 'ECONNRESET' }))
    await waitFor(() => expect(res.destroyed).toBe(true))
    expect(mocks.build.mock.results[0]!.value.close).toHaveBeenCalledTimes(1)
    expect(mocks.fetch).toHaveBeenCalledTimes(1)
    await startNativeMitm(opts)
    expect(mocks.build).toHaveBeenCalledTimes(2)
  },
)

it('interrupts a paused request and retires its pool without aborting a neighbor', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  const opts = { ptySessionId: 'interrupt', onEntryUpdate: vi.fn() }
  const handle = await startNativeMitm(opts)
  const server = mocks.servers.at(-1)!
  await startNativeMitm({ ...opts, ptySessionId: 'neighbor' })
  mocks.fetch.mockImplementation(async () =>
    streamResponse(
      new ReadableStream({
        start(c) {
          c.enqueue(Buffer.alloc(64 * 1024, 32))
        },
      }),
    ),
  )
  const res = request(server, true)
  const neighbor = request()
  await waitFor(() => expect(mocks.fetch).toHaveBeenCalledTimes(2))
  await waitFor(() => expect(res.writableLength).toBeGreaterThan(0))
  handle.interrupt()
  await waitFor(() => expect(res.destroyed).toBe(true))
  expect(mocks.fetch.mock.calls[0]![1].signal.aborted).toBe(true)
  expect(mocks.fetch.mock.calls[1]![1].signal.aborted).toBe(false)
  expect(neighbor.destroyed).toBe(false)
  expect(mocks.build.mock.results[0]!.value.close).toHaveBeenCalledTimes(1)
  expect(res.listenerCount('drain')).toBe(0)
  expect(mocks.fetch).toHaveBeenCalledTimes(2)
})

it('aborts and logs an oversized chunk explicitly without recycling the pool', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  await startNativeMitm({ ptySessionId: 'budget', onEntryUpdate: vi.fn() })
  mocks.fetch.mockResolvedValue(
    streamResponse(
      new ReadableStream({
        start(c) {
          c.enqueue(Buffer.alloc(256 * 1024 + 1))
          c.close()
        },
      }),
    ),
  )
  const res = request()
  await waitFor(() => expect(res.destroyed).toBe(true))
  expect(mocks.fetch.mock.calls[0]![1].signal.aborted).toBe(true)
  expect(res.errored).toMatchObject({ code: 'MITM_RESPONSE_BUFFER_LIMIT' })
  expect(mocks.log).toHaveBeenCalledWith(
    '[mitm] response buffer limit exceeded',
    expect.objectContaining({
      reason: 'MITM_RESPONSE_BUFFER_LIMIT',
      limitBytes: 262144,
      chunkBytes: 262145,
    }),
  )
  expect(mocks.build.mock.results[0]!.value.close).not.toHaveBeenCalled()
  expect(mocks.fetch).toHaveBeenCalledTimes(1)
})

it('a late interrupted old response cannot retire the replacement pool', async () => {
  const { startNativeMitm, resetExtraTrustRoots } = await import('./native-mitm')
  const opts = { ptySessionId: 'old-interrupt', onEntryUpdate: vi.fn() }
  await startNativeMitm(opts)
  mocks.fetch.mockResolvedValue(
    streamResponse(
      new ReadableStream({
        start(c) {
          c.enqueue(Buffer.alloc(64 * 1024, 32))
        },
      }),
    ),
  )
  const res = request(undefined, true)
  await waitFor(() => expect(res.writableLength).toBeGreaterThan(0))
  resetExtraTrustRoots()
  await startNativeMitm({ ...opts, ptySessionId: 'replacement' })
  const replacement = mocks.build.mock.results[1]!.value
  res.destroy()
  await waitFor(() =>
    expect(mocks.log).toHaveBeenCalledWith(
      '[mitm][pool] stale reset ignored',
      expect.objectContaining({ reason: 'client-interrupt', generation: 1, currentGeneration: 2 }),
    ),
  )
  expect(replacement.close).not.toHaveBeenCalled()
  expect(mocks.fetch).toHaveBeenCalledTimes(1)
})

it('resumes on drain, preserving raw heartbeats and synthesis through clean completion', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  const onEntryUpdate = vi.fn()
  await startNativeMitm({ ptySessionId: 'drain', onEntryUpdate })
  const events = [
    ':' + ' '.repeat(64 * 1024) + '\n\n',
    'event: message_start\ndata: {"type":"message_start","message":{"id":"msg-test","model":"claude-sonnet-4","role":"assistant","content":[]}}\n\n',
    'event: message_stop\ndata: {"type":"message_stop"}\n\n',
  ]
  mocks.fetch.mockResolvedValue(
    streamResponse(
      new ReadableStream({
        start(c) {
          for (const e of events) c.enqueue(Buffer.from(e))
          c.close()
        },
      }),
    ),
  )
  const res = request(undefined, true)
  await waitFor(() => expect(res.writableLength).toBeGreaterThan(0))
  expect(res.writableFinished).toBe(false)
  res.unblock()
  await waitFor(() => expect(res.writableFinished).toBe(true))
  expect(Buffer.concat(res.chunks).toString()).toBe(events.join(''))
  expect(onEntryUpdate).toHaveBeenCalled()
  expect(mocks.build.mock.results[0]!.value.close).not.toHaveBeenCalled()
})
