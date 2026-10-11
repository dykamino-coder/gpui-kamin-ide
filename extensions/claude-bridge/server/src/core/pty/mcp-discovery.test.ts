// In-memory HTTP routing only: no Bridge listener or Claude process is started.
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { Hono } from 'hono'
import type { PtySession } from './types'

const mocks = vi.hoisted(() => ({ getSession: vi.fn(), emit: vi.fn() }))
vi.mock('./session-manager', () => ({ getSession: mocks.getSession, sendMcpCall: vi.fn() }))
vi.mock('./session-ws', () => ({ getSessionWs: vi.fn() }))
vi.mock('./session-io', () => ({ sendToClient: vi.fn() }))
vi.mock('../events/bus', () => ({ eventBus: { emit: mocks.emit } }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn() }))

import { handleMcpRequest } from './mcp-http-handler'

const app = new Hono().post('/mcp/:sessionId', handleMcpRequest)
let session: PtySession

function request(method: string, id: string | number = 1, token = 'synthetic-token') {
  const params =
    method === 'server/discover'
      ? {
          _meta: {
            'io.modelcontextprotocol/protocolVersion': '2026-07-28',
            'io.modelcontextprotocol/clientInfo': { name: 'claude-code', version: '2.1.296' },
            'io.modelcontextprotocol/clientCapabilities': {},
          },
        }
      : {}
  return app.request('/mcp/synthetic-session', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      authorization: `Bearer ${token}`,
      'MCP-Protocol-Version': method === 'server/discover' ? '2026-07-28' : '2025-11-25',
      'Mcp-Method': method,
    },
    body: JSON.stringify({ jsonrpc: '2.0', id, method, params }),
  })
}

beforeEach(() => {
  vi.clearAllMocks()
  session = {
    id: 'synthetic-session',
    mcpToken: 'synthetic-token',
    mcpInitialized: false,
    mcpLastError: null,
    mcpLog: [],
    mcpCallCount: 0,
    lastActivityAt: new Date(),
    registeredTools: [],
  } as unknown as PtySession
  mocks.getSession.mockReturnValue(session)
})

describe('legacy MCP discovery fallback', () => {
  it.each(['server-discover-probe-1', 1, 0])('rejects probe %s without a sticky session error', async (id) => {
    const response = await request('server/discover', id)
    expect(response.status).toBe(400)
    expect(await response.json()).toEqual({
      jsonrpc: '2.0',
      id,
      error: { code: -32601, message: 'Method not found: server/discover' },
    })
    expect(response.headers.get('Mcp-Session-Id')).toBe(session.id)
    expect(session.mcpInitialized).toBe(false)
    expect(session.mcpLastError).toBeNull()
    expect(session.mcpLog).toEqual([expect.objectContaining({ method: 'server/discover', status: 'error' })])
  })

  it('completes initialize and tools/list after a discovery probe', async () => {
    await request('server/discover', 'server-discover-probe-1')
    const initialized = await request('initialize', 2)
    expect(initialized.status).toBe(200)
    expect((await initialized.json()).result.capabilities.tools).toEqual({})
    expect(session.mcpInitialized).toBe(true)
    expect(mocks.emit).toHaveBeenCalledWith('session:updated', expect.objectContaining({ mcpLastError: null }))
    const listed = await request('tools/list', 3)
    expect(listed.status).toBe(200)
    expect((await listed.json()).result.tools.length).toBeGreaterThan(0)
    expect(session.mcpLastError).toBeNull()
  })

  it('preserves an earlier real error across discovery and successful initialization', async () => {
    session.mcpLastError = 'Earlier tool failure'
    await request('server/discover')
    await request('initialize', 2)
    expect(session.mcpLastError).toBe('Earlier tool failure')
  })

  it('still records unknown methods and invalid tool calls as real errors', async () => {
    const unknown = await request('server/unknown')
    expect((await unknown.json()).error.code).toBe(-32601)
    expect(session.mcpLastError).toBe('Method not found: server/unknown')
    await request('server/discover', 2)
    expect(session.mcpLastError).toBe('Method not found: server/unknown')
    await request('tools/call', 3)
    expect(session.mcpLastError).toBe('Missing tool name')
  })

  it('does not bypass authentication for discovery', async () => {
    expect((await request('server/discover', 1, 'invalid-token')).status).toBe(401)
    expect(session.mcpLog).toEqual([])
  })
})
