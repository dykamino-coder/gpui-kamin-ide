// INC-2026-0121: Claude CLI spills an MCP result above the tool's result size
// limit (50,000 characters by default) into a file in this container, which the
// client-side Read cannot open. Every listed tool declares the CLI's 500K
// ceiling, and results are cut below it. In-memory HTTP routing only.
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { Hono } from 'hono'
import type { PtySession } from './types'

const mocks = vi.hoisted(() => ({ getSession: vi.fn(), call: vi.fn() }))
vi.mock('./session-manager', () => ({ getSession: mocks.getSession, sendMcpCall: mocks.call }))
vi.mock('./session-ws', () => ({ getSessionWs: vi.fn() }))
vi.mock('./session-io', () => ({ sendToClient: vi.fn() }))
vi.mock('../events/bus', () => ({ eventBus: { emit: vi.fn() } }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn() }))

import { handleMcpRequest, truncateMcpResult, withResultSize } from './mcp-http-handler'

const app = new Hono().post('/mcp/:sessionId', handleMcpRequest)
let session: PtySession

async function rpc(method: string, params: Record<string, unknown> = {}): Promise<any> {
  const response = await app.request('/mcp/synthetic-session', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json, text/event-stream',
      authorization: 'Bearer synthetic-token',
    },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  })
  const text = await response.text()
  const json = text.startsWith('{')
    ? text
    : text
        .split('\n')
        .find((line) => line.startsWith('data: '))!
        .slice(6)
  return JSON.parse(json)
}

beforeEach(() => {
  vi.clearAllMocks()
  session = {
    id: 'synthetic-session',
    mcpToken: 'synthetic-token',
    mcpInitialized: true,
    mcpLastError: null,
    mcpLog: [],
    mcpCallCount: 0,
    lastActivityAt: new Date(),
    registeredTools: [{ name: 'external_dump', description: 'x', inputSchema: { type: 'object' } }],
    ws: { readyState: 1 },
  } as unknown as PtySession
  mocks.getSession.mockReturnValue(session)
})

describe('result size of forwarded tools', () => {
  it('declares the 500K inline limit on every listed tool, external ones included', async () => {
    const tools = (await rpc('tools/list')).result.tools as Array<{ name: string; _meta?: Record<string, unknown> }>
    expect(tools.length).toBeGreaterThan(10)
    for (const tool of tools) expect(tool._meta?.['anthropic/maxResultSizeChars'], tool.name).toBe(500_000)
    expect(tools.map((tool) => tool.name)).toContain('external_dump')
  })

  it('keeps metadata a tool already declares', () => {
    expect(withResultSize({ name: 'a', _meta: { other: 1 } })._meta).toEqual({
      other: 1,
      'anthropic/maxResultSizeChars': 500_000,
    })
  })

  it('cuts a result below the declared limit and says so', () => {
    const cut = truncateMcpResult({ content: [{ type: 'text', text: 'x'.repeat(700_000) }] }, 'Bash')
    const text = (cut.content as Array<{ text: string }>).map((block) => block.text).join('')
    expect(text.length).toBeLessThanOrEqual(500_000)
    expect(text).toContain('[Output truncated by bridge')
  })

  it('passes a 300K result through a forwarded call unchanged', async () => {
    const big = 'y'.repeat(300_000)
    mocks.call.mockResolvedValue({ content: [{ type: 'text', text: big }] })
    const answer = await rpc('tools/call', { name: 'Read', arguments: { file_path: '/p/a' } })
    expect(answer.result.content[0].text).toBe(big)
  })
})
