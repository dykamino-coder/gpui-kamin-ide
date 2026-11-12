import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { TransportContext } from './context'
import { fetchDeadline } from './fetch-deadline'
import { MCP_REQUEST_TIMEOUT_MS, MCP_TOOL_CALL_IDLE_TIMEOUT_MS, MCP_TOOL_CALL_MAX_TIMEOUT_MS } from './request-deadline'

// Scaled deadlines so a real local server can answer "late" in milliseconds:
// tool calls 1.5 s total / 400 ms idle, service requests 250 ms.
vi.mock('./request-deadline', async (original) => ({
  ...await original<typeof import('./request-deadline')>(),
  mcpRequestDeadline: (method: string) => method === 'tools/call'
    ? { idleMs: 400, maxMs: 1_500 }
    : { idleMs: 250, maxMs: 250 },
}))
vi.mock('../oauth-store', () => ({ loadTokens: () => null }))

const { callHttpTool, httpJsonRpcAuth } = await import('./http')

const UNDICI_DEFAULT_TIMEOUT_MS = 300_000
let server: Server | undefined
afterEach(() => new Promise<void>((resolve) => {
  if (!server) return resolve()
  server.closeAllConnections()
  server.close(() => resolve())
}))

type Responder = (body: any, res: import('node:http').ServerResponse) => void
async function mcpServer(respond: Responder): Promise<TransportContext> {
  server = createServer((req, res) => {
    let raw = ''
    req.on('data', (chunk) => { raw += chunk })
    req.on('end', () => respond(JSON.parse(raw), res))
  })
  await new Promise<void>((resolve) => server!.listen(0, '127.0.0.1', resolve))
  const url = `http://127.0.0.1:${(server.address() as AddressInfo).port}/mcp`
  return {
    servers: new Map([['remote', { config: { url }, nextRequestId: 1 }]]),
    buildHttpHeaders: async () => ({}),
    handleServerMessage: () => {},
  } as unknown as TransportContext
}

const later = (ms: number, run: () => void) => setTimeout(run, ms)
const json = (res: import('node:http').ServerResponse, body: unknown) => {
  res.writeHead(200, { 'content-type': 'application/json' })
  res.end(JSON.stringify(body))
}

describe('Streamable HTTP request deadlines', () => {
  it('lifts tool calls above fetch\'s own 300 s limits and keeps service requests at 120 s', () => {
    expect(fetchDeadline('tools/call')).toEqual({ headersMs: 1_500, bodyMs: 400 }) // scaled by the mock
    expect(MCP_TOOL_CALL_MAX_TIMEOUT_MS).toBeGreaterThan(UNDICI_DEFAULT_TIMEOUT_MS)
    expect(MCP_TOOL_CALL_IDLE_TIMEOUT_MS).toBeGreaterThan(UNDICI_DEFAULT_TIMEOUT_MS)
    expect(MCP_REQUEST_TIMEOUT_MS).toBe(120_000)
  })

  it('accepts a JSON tool result that arrives after the service deadline', async () => {
    const ctx = await mcpServer((body, res) => later(700, () =>
      json(res, { jsonrpc: '2.0', id: body.id, result: { content: [{ type: 'text', text: 'star' }] } })))
    await expect(callHttpTool(ctx, 'remote', 'choose_icon', {}))
      .resolves.toEqual({ content: [{ type: 'text', text: 'star' }] })
  })

  it('applies the short header deadline to a service request', async () => {
    const ctx = await mcpServer((body, res) => later(700, () => json(res, { jsonrpc: '2.0', id: body.id, result: {} })))
    await expect(httpJsonRpcAuth(ctx, 'remote', { jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} }))
      .rejects.toThrow(/Headers Timeout|timeout/i)
  })

  it('keeps an SSE tool reply alive while progress events stream, within the total cap', async () => {
    const ctx = await mcpServer((body, res) => {
      res.writeHead(200, { 'content-type': 'text/event-stream' })
      let progress = 0
      const tick = setInterval(() => {
        progress += 1
        res.write(`data: ${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/progress', params: { progressToken: body.params._meta.progressToken, progress } })}\n\n`)
        if (progress === 4) {
          clearInterval(tick)
          res.end(`data: ${JSON.stringify({ jsonrpc: '2.0', id: body.id, result: { content: [] } })}\n\n`)
        }
      }, 250)
    })
    await expect(callHttpTool(ctx, 'remote', 'choose_icon', {})).resolves.toEqual({ content: [] })
  })

  it('stops an SSE tool reply that falls silent past the idle deadline', async () => {
    const ctx = await mcpServer((_body, res) => {
      res.writeHead(200, { 'content-type': 'text/event-stream' })
      res.write(': started\n\n')
    })
    const started = Date.now()
    await expect(callHttpTool(ctx, 'remote', 'choose_icon', {})).rejects.toThrow(/terminated/i)
    // fetch's default body timeout is 300 s and the total cap 1.5 s: only the
    // composed dispatcher can end the call this early.
    expect(Date.now() - started).toBeLessThan(1_200)
  })

  it('stops an SSE tool reply at the total cap even while progress keeps arriving', async () => {
    const ctx = await mcpServer((_body, res) => {
      res.writeHead(200, { 'content-type': 'text/event-stream' })
      const tick = setInterval(() => res.write(': progress\n\n'), 200)
      res.on('close', () => clearInterval(tick))
    })
    const started = Date.now()
    await expect(callHttpTool(ctx, 'remote', 'choose_icon', {})).rejects.toThrow(/abort|timeout/i)
    expect(Date.now() - started).toBeGreaterThanOrEqual(1_400)
  })
})
