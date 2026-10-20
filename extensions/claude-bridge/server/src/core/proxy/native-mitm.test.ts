// Exercise the production request handler without opening host listeners or TLS sockets.
import { Buffer } from 'node:buffer'
import { EventEmitter } from 'node:events'
import type { IncomingMessage, ServerResponse } from 'node:http'
import { Readable, Writable } from 'node:stream'
import { ReadableStream, type ReadableStreamDefaultController } from 'node:stream/web'
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
  vi.restoreAllMocks()
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

// Observe explicit flow control separately from Node's initial automatic resume
// when the production handler attaches its first data listener.
function observedUpstream(body: ReadableStream<Uint8Array>) {
  const src = Readable.fromWeb(body)
  const pause = vi.spyOn(src, 'pause')
  const resume = vi.spyOn(src, 'resume')
  vi.spyOn(Readable, 'fromWeb').mockReturnValueOnce(src)
  return { src, pause, resume }
}

function heartbeat(bytes = 64 * 1024) {
  const chunk = Buffer.alloc(bytes, 32)
  chunk[0] = 58
  chunk.write('\n\n', chunk.length - 2)
  return chunk
}

it('mitm-no-drain aborts a 20 MiB SSE response at the 16 MiB cap without throttling upstream', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  await startNativeMitm({ ptySessionId: 'blocked', onEntryUpdate: vi.fn() })
  let pulls = 0
  const chunk = heartbeat()
  const cancel = vi.fn()
  const body = new ReadableStream({
    pull(controller) {
      if (pulls++ < 320) controller.enqueue(chunk)
      else controller.close()
    },
    cancel,
  })
  const flow = observedUpstream(body)
  mocks.fetch.mockResolvedValue(streamResponse(body))
  const res = request(undefined, true)
  await waitFor(() => expect(res.destroyed).toBe(true))
  expect(res.errored).toMatchObject({ code: 'MITM_RESPONSE_BUFFER_LIMIT' })
  expect(mocks.log).toHaveBeenCalledWith(
    '[mitm] response buffer limit exceeded',
    expect.objectContaining({ reason: 'MITM_RESPONSE_BUFFER_LIMIT', limitBytes: 16 * 1024 * 1024 }),
  )
  expect(pulls).toBeGreaterThanOrEqual(256)
  expect(pulls).toBeLessThan(320)
  expect(cancel).toHaveBeenCalledTimes(1)
  expect(flow.pause).not.toHaveBeenCalled()
  expect(flow.resume).toHaveBeenCalledTimes(1) // initial data-listener activation only
  expect(mocks.fetch.mock.calls[0]![1].signal.reason).toMatchObject({ code: 'MITM_RESPONSE_BUFFER_LIMIT' })
  expect(mocks.build.mock.results[0]!.value.close).not.toHaveBeenCalled()
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

it('interrupts a queued request and retires its pool without aborting a neighbor', async () => {
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
  await waitFor(() => expect(mocks.build.mock.results[0]!.value.close).toHaveBeenCalledTimes(1))
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
          c.enqueue(heartbeat(16 * 1024 * 1024 + 1))
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
      limitBytes: 16 * 1024 * 1024,
      chunkBytes: 16 * 1024 * 1024 + 1,
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

it('flushes a slow consumer below the cap after upstream EOF without pause/resume or truncation', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  const onEntryUpdate = vi.fn()
  await startNativeMitm({ ptySessionId: 'drain', onEntryUpdate })
  const events = [
    heartbeat().toString().repeat(32),
    'event: message_start\ndata: {"type":"message_start","message":{"id":"msg-test","model":"claude-sonnet-4","role":"assistant","content":[]}}\n\n',
    'event: message_stop\ndata: {"type":"message_stop"}\n\n',
  ]
  const body = new ReadableStream<Uint8Array>({
    start(c) {
      for (const e of events) c.enqueue(Buffer.from(e))
      c.close()
    },
  })
  const flow = observedUpstream(body)
  mocks.fetch.mockResolvedValue(streamResponse(body))
  const res = request(undefined, true)
  await waitFor(() => expect(res.writableLength).toBeGreaterThan(0))
  await waitFor(() => expect(flow.src.readableEnded).toBe(true))
  expect(res.writableFinished).toBe(false)
  expect(flow.pause).not.toHaveBeenCalled()
  expect(flow.resume).toHaveBeenCalledTimes(1)
  flow.resume.mockClear()
  res.unblock()
  await waitFor(() => expect(res.writableFinished).toBe(true))
  expect(Buffer.concat(res.chunks).toString()).toBe(events.join(''))
  expect(flow.pause).not.toHaveBeenCalled()
  expect(flow.resume).not.toHaveBeenCalled()
  expect(res.listenerCount('drain')).toBe(0)
  expect(onEntryUpdate).toHaveBeenCalled()
  expect(mocks.build.mock.results[0]!.value.close).not.toHaveBeenCalled()
})

it('enforces the process-wide 256 MiB cap across responses and releases capacity on abort/finish', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  await startNativeMitm({ ptySessionId: 'global-budget', onEntryUpdate: vi.fn() })
  const chunk = heartbeat(1024 * 1024)
  const controllers: ReadableStreamDefaultController<Uint8Array>[] = []
  mocks.fetch.mockImplementation(async () =>
    streamResponse(
      new ReadableStream({
        start(c) {
          controllers.push(c)
        },
      }),
    ),
  )
  const blocked: CliResponse[] = []
  // Sixteen responses each retain exactly 16 MiB, including the first write
  // pending inside res. All are healthy at the exact per-response/global caps.
  for (let i = 0; i < 16; i++) {
    const res = request(undefined, true)
    blocked.push(res)
    await waitFor(() => expect(res.listenerCount('drain')).toBe(1))
    for (let n = 0; n < 16; n++) controllers[i]!.enqueue(chunk)
    controllers[i]!.close()
    await waitFor(() =>
      expect(mocks.log.mock.calls.filter(([msg]) => msg === '[mitm][diag#73] SSE turn done')).toHaveLength(i + 1),
    )
    expect(res.destroyed).toBe(false)
  }
  const rejected = request(undefined, true)
  await waitFor(() => expect(rejected.listenerCount('drain')).toBe(1))
  controllers[16]!.enqueue(chunk)
  await waitFor(() => expect(rejected.destroyed).toBe(true))
  expect(rejected.errored).toMatchObject({ code: 'MITM_GLOBAL_BUFFER_LIMIT' })
  expect(mocks.log).toHaveBeenCalledWith(
    '[mitm] response buffer limit exceeded',
    expect.objectContaining({
      reason: 'MITM_GLOBAL_BUFFER_LIMIT',
      totalQueuedBytes: 256 * 1024 * 1024,
      limitBytes: 256 * 1024 * 1024,
    }),
  )
  expect(blocked.every((res) => !res.destroyed)).toBe(true)
  // Closing an already-ended upstream's blocked CLI must release its queue.
  blocked[0]!.destroy()
  await waitFor(() => expect(blocked[0]!.listenerCount('drain')).toBe(0))
  blocked[0]!.unblock() // late write callback after release must be ignored
  const accepted = request(undefined, true)
  await waitFor(() => expect(accepted.listenerCount('drain')).toBe(1))
  for (let n = 0; n < 16; n++) controllers[17]!.enqueue(chunk)
  controllers[17]!.close()
  await waitFor(() =>
    expect(mocks.log.mock.calls.filter(([msg]) => msg === '[mitm][diag#73] SSE turn done')).toHaveLength(17),
  )
  expect(accepted.destroyed).toBe(false)
  const lateCallbackProbe = request(undefined, true)
  await waitFor(() => expect(lateCallbackProbe.listenerCount('drain')).toBe(1))
  controllers[18]!.enqueue(chunk)
  await waitFor(() => expect(lateCallbackProbe.destroyed).toBe(true))
  expect(lateCallbackProbe.errored).toMatchObject({ code: 'MITM_GLOBAL_BUFFER_LIMIT' })
  // Successful drains must release pending writes as well as queued chunks.
  for (const res of [...blocked.slice(1), accepted]) res.unblock()
  await waitFor(() => expect(accepted.writableFinished).toBe(true))
  expect(Buffer.concat(accepted.chunks).length).toBe(16 * 1024 * 1024)
  expect(mocks.build.mock.results[0]!.value.close).not.toHaveBeenCalled()
  // A 16 MiB newcomer fits only if successful drains release reservations.
  const finishedProbe = request(undefined, true)
  await waitFor(() => expect(finishedProbe.listenerCount('drain')).toBe(1))
  for (let n = 0; n < 16; n++) controllers[19]!.enqueue(chunk)
  controllers[19]!.close()
  await waitFor(() =>
    expect(mocks.log.mock.calls.filter(([msg]) => msg === '[mitm][diag#73] SSE turn done')).toHaveLength(18),
  )
  expect(finishedProbe.destroyed).toBe(false)
  expect(mocks.fetch).toHaveBeenCalledTimes(20)
})

it('interrupts pending CLI delivery even after upstream EOF without retry', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  const handle = await startNativeMitm({ ptySessionId: 'interrupt-after-eof', onEntryUpdate: vi.fn() })
  const body = new ReadableStream<Uint8Array>({
    start(c) {
      c.enqueue(heartbeat())
      c.enqueue(heartbeat())
      c.close()
    },
  })
  const flow = observedUpstream(body)
  mocks.fetch.mockResolvedValue(streamResponse(body))
  const res = request(undefined, true)
  await waitFor(() => expect(flow.src.readableEnded).toBe(true))
  handle.interrupt()
  await waitFor(() => expect(res.destroyed).toBe(true))
  expect(res.writableFinished).toBe(false)
  expect(res.errored).toMatchObject({ name: 'AbortError' })
  expect(res.listenerCount('drain')).toBe(0)
  expect(flow.pause).not.toHaveBeenCalled()
  expect(mocks.fetch).toHaveBeenCalledTimes(1)
})

it('aborts a CLI write exception while draining after upstream EOF', async () => {
  const { startNativeMitm } = await import('./native-mitm')
  await startNativeMitm({ ptySessionId: 'write-after-eof', onEntryUpdate: vi.fn() })
  const body = new ReadableStream<Uint8Array>({
    start(c) {
      c.enqueue(heartbeat())
      c.enqueue(heartbeat())
      c.close()
    },
  })
  const flow = observedUpstream(body)
  mocks.fetch.mockResolvedValue(streamResponse(body))
  const res = request(undefined, true)
  await waitFor(() => expect(flow.src.readableEnded).toBe(true))
  const failure = new Error('synthetic CLI write failure')
  vi.spyOn(res, 'write').mockImplementationOnce(() => {
    throw failure
  })
  res.unblock()
  await waitFor(() => expect(res.destroyed).toBe(true))
  expect(res.errored).toBe(failure)
  expect(mocks.fetch.mock.calls[0]![1].signal.reason).toBe(failure)
  expect(res.listenerCount('drain')).toBe(0)
  expect(mocks.fetch).toHaveBeenCalledTimes(1)
})
