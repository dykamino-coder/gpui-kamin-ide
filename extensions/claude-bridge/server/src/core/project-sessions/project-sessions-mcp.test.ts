// INC-2026-0072 through the real MCP HTTP handler: the tools are advertised,
// run on the server (never forwarded to the client host), take their scope from
// the authenticated session — real SESSIONS_BASE and sanitizeDirName — and leave
// a content-free access record in the lifecycle journal.
import 'reflect-metadata'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import { Hono } from 'hono'
import type { PtySession } from '../pty/types'

const mocks = vi.hoisted(() => ({ sessions: new Map<string, unknown>(), sendMcpCall: vi.fn() }))
vi.mock('../pty/session-manager', () => ({
  getSession: (id: string) => mocks.sessions.get(id),
  getAllSessions: () => [...mocks.sessions.values()],
  sendMcpCall: mocks.sendMcpCall,
}))
vi.mock('../pty/session-ws', () => ({ getSessionWs: vi.fn() }))
vi.mock('../pty/session-io', () => ({ sendToClient: vi.fn() }))
vi.mock('../events/bus', () => ({ eventBus: { emit: vi.fn() } }))
vi.mock('../stats/database/lifecycle', () => ({ getDataDir: () => path.join(os.tmpdir(), 'projsess-no-archive') }))

const TOKEN = '6f1c2d7e-0000-4000-8000-0000000000aa'
const OTHER = '6f1c2d7e-0000-4000-8000-0000000000bb'
const CWD = 'C:\\Users\\alice\\Projects\\app'
const MINE = 'aaaaaaaa-1111-4000-8000-00000000000a'
const NEIGHBOUR = 'bbbbbbbb-1111-4000-8000-00000000000b'
const FOREIGN = 'ffffffff-1111-4000-8000-00000000000f'

let home = ''
let lifecycleDir = ''
const env = { HOME: process.env.HOME, USERPROFILE: process.env.USERPROFILE, LC: process.env.BRIDGE_LIFECYCLE_LOG_DIR }
let app: Hono

function seed(tokenId: string, sessionId: string, conversationId: string, prompt: string): string {
  const base = path.join(home, '.claude', 'bridge-sessions')
  const settingsDir = path.join(base, `${tokenId}-C_Users_alice_Projects_app`, sessionId)
  fs.mkdirSync(settingsDir, { recursive: true })
  const slug = path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-')
  const dir = path.join(home, '.claude', 'projects', slug)
  fs.mkdirSync(dir, { recursive: true })
  const entry = { type: 'user', timestamp: '2026-10-20T10:00:00.000Z', message: { role: 'user', content: prompt } }
  fs.writeFileSync(path.join(dir, `${conversationId}.jsonl`), JSON.stringify(entry) + '\n')
  return settingsDir
}

function session(id: string, tokenId: string, settingsDir: string, conversationId: string): PtySession {
  const s = {
    id,
    tokenId,
    userName: 'alice',
    cwd: CWD,
    settingsDir,
    cliConversationId: conversationId,
    state: 'running',
    mcpToken: `mcp-${id}`,
    mcpInitialized: true,
    mcpCallCount: 0,
    mcpLastError: null,
    mcpLog: [],
    lastActivityAt: new Date(0),
    registeredTools: [],
    registeredResources: [],
    registeredResourceTemplates: [],
    registeredPrompts: [],
  } as unknown as PtySession
  mocks.sessions.set(id, s)
  return s
}

async function rpc(sessionId: string, method: string, params: Record<string, unknown> = {}) {
  const res = await app.request(`/mcp/${sessionId}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer mcp-${sessionId}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  })
  const raw = await res.text()
  const data = raw.includes('event: message') ? raw.split('data: ').pop()! : raw
  return JSON.parse(data) as {
    result: { tools?: Array<{ name: string }>; content?: Array<{ text: string }>; isError?: boolean }
  }
}

beforeAll(async () => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), 'projsess-home-'))
  lifecycleDir = path.join(home, 'lifecycle')
  process.env.HOME = home
  process.env.USERPROFILE = home
  process.env.BRIDGE_LIFECYCLE_LOG_DIR = lifecycleDir
  vi.resetModules()
  const { handleMcpRequest } = await import('../pty/mcp-http-handler')
  app = new Hono().post('/mcp/:sessionId', handleMcpRequest)
  session('s-mine', TOKEN, seed(TOKEN, 'sess-1', MINE, 'my current chat'), MINE)
  seed(TOKEN, 'sess-2', NEIGHBOUR, 'neighbour history to restore')
  session('s-other', OTHER, seed(OTHER, 'sess-3', FOREIGN, 'other user private text'), FOREIGN)
})

afterAll(() => {
  process.env.HOME = env.HOME
  process.env.USERPROFILE = env.USERPROFILE
  if (env.LC === undefined) delete process.env.BRIDGE_LIFECYCLE_LOG_DIR
  else process.env.BRIDGE_LIFECYCLE_LOG_DIR = env.LC
  fs.rmSync(home, { recursive: true, force: true })
})

describe('project session tools over MCP', () => {
  it('advertises all seven tools', async () => {
    const names = (await rpc('s-mine', 'tools/list')).result.tools!.map((t) => t.name)
    expect(names).toEqual(
      expect.arrayContaining([
        'project_sessions_list',
        'project_session_read',
        'project_session_raw',
        'project_session_search',
        'project_session_outline',
        'project_session_subagents',
        'project_session_brief',
      ]),
    )
  })

  it('lists and reads the neighbour on the server and refuses the other token', async () => {
    const list = await rpc('s-mine', 'tools/call', { name: 'project_sessions_list', arguments: {} })
    const out = JSON.parse(list.result.content![0]!.text)
    expect(out.project).toBe(CWD)
    expect(out.conversations.map((c: { conversation_id: string }) => c.conversation_id).sort()).toEqual(
      [MINE, NEIGHBOUR].sort(),
    )
    expect(out.conversations.find((c: { self: boolean }) => c.self).conversation_id).toBe(MINE)

    const read = await rpc('s-mine', 'tools/call', {
      name: 'project_session_read',
      arguments: { conversation_id: NEIGHBOUR },
    })
    expect(read.result.content![0]!.text).toContain('neighbour history to restore')

    const foreign = await rpc('s-mine', 'tools/call', {
      name: 'project_session_read',
      arguments: { conversation_id: FOREIGN },
    })
    expect(foreign.result.isError).toBe(true)
    expect(JSON.stringify(foreign)).not.toContain('other user private text')
    expect(mocks.sendMcpCall).not.toHaveBeenCalled()
  })

  it('journals access metadata without content', async () => {
    const journal = fs.readFileSync(path.join(lifecycleDir, 'lifecycle.jsonl'), 'utf8')
    const records = journal
      .trim()
      .split('\n')
      .map((l) => JSON.parse(l) as Record<string, unknown>)
      .filter((r) => r.event === 'project_session_access')
    expect(records.map((r) => [r.tool, r.outcome])).toEqual([
      ['project_sessions_list', 'ok'],
      ['project_session_read', 'ok'],
      ['project_session_read', 'not_found'],
    ])
    expect(records[1]).toMatchObject({ self: false, conversationRef: expect.stringMatching(/^[0-9a-f]{12}$/) })
    expect(records[1]!.bytes).toBeGreaterThan(0)
    expect(records[1]!.tokenRef).toMatch(/^[0-9a-f]{12}$/)
    expect(journal).not.toContain('neighbour history')
    expect(journal).not.toContain(NEIGHBOUR)
    expect(journal).not.toContain(TOKEN)
  })

  it('can be switched off', async () => {
    process.env.KAMIN_PROJECT_SESSION_TOOLS = 'off'
    try {
      const names = (await rpc('s-mine', 'tools/list')).result.tools!.map((t) => t.name)
      expect(names).not.toContain('project_sessions_list')
      const call = await rpc('s-mine', 'tools/call', { name: 'project_sessions_list', arguments: {} })
      expect(JSON.stringify(call)).toContain('Unknown tool')
    } finally {
      delete process.env.KAMIN_PROJECT_SESSION_TOOLS
    }
  })
})
