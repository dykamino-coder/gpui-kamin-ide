import { afterEach, expect, it, vi } from 'vitest'
import type { TransportContext } from './context'
import { sseJsonRpcRequest } from './sse-legacy'

afterEach(() => vi.unstubAllGlobals())

function fixture() {
  const shutdown = new AbortController()
  const pendingRequests = new Map<number, {
    resolve: (value: unknown) => void
    reject: (error: Error) => void
    timer?: ReturnType<typeof setTimeout>
  }>()
  const context = {
    shutdownSignal: shutdown.signal,
    servers: new Map([['fixture', { sseMessageEndpoint: 'http://fixture.invalid/message', pendingRequests }]]),
    buildHttpHeaders: async () => ({}),
  } as unknown as TransportContext
  return { shutdown, pendingRequests, context }
}

it('observes pending SSE replies while final shutdown aborts a POST awaiting headers', async () => {
  const { shutdown, pendingRequests, context } = fixture()
  let entered!: () => void
  const started = new Promise<void>(resolve => { entered = resolve })
  let aborted = false
  vi.stubGlobal('fetch', vi.fn((_url: string, options: RequestInit) => new Promise((_resolve, reject) => {
    options.signal!.addEventListener('abort', () => {
      aborted = true
      reject(new Error('POST aborted'))
    }, { once: true })
    entered()
  })))
  const call = sseJsonRpcRequest(context, 'fixture', { id: 1, method: 'tools/call' })
  const rejected = expect(call).rejects.toThrow()
  await started
  // The manager aborts transport I/O and rejects its pending JSON-RPC replies.
  shutdown.abort()
  pendingRequests.get(1)!.reject(new Error('Server disconnected'))
  await rejected
  expect(aborted).toBe(true)
  expect(pendingRequests.size).toBe(0)
})

it('cleans the reply timer when an SSE POST fails before a stream reply', async () => {
  const { pendingRequests, context } = fixture()
  vi.stubGlobal('fetch', vi.fn(async () => new Response('fixture rejection', { status: 503 })))
  await expect(sseJsonRpcRequest(context, 'fixture', { id: 2, method: 'tools/call' }))
    .rejects.toThrow('SSE POST 503')
  expect(pendingRequests.size).toBe(0)
})

it('cancels a reply after POST 202 without cancelling another call on the stream', async () => {
  const { pendingRequests, context } = fixture()
  const owner = new AbortController()
  vi.stubGlobal('fetch', vi.fn(async () => new Response(null, { status: 202 })))
  const cancelled = sseJsonRpcRequest({ ...context, shutdownSignal: owner.signal }, 'fixture', { id: 1, method: 'tools/call' })
  const rejected = expect(cancelled).rejects.toThrow('MCP request cancelled')
  const other = sseJsonRpcRequest(context, 'fixture', { id: 2, method: 'tools/call' })
  await vi.waitFor(() => expect(pendingRequests.size).toBe(2))
  owner.abort()
  await rejected
  expect(pendingRequests.has(1)).toBe(false)
  pendingRequests.get(2)!.resolve('other result')
  await expect(other).resolves.toBe('other result')
  expect(pendingRequests.size).toBe(0)
})
