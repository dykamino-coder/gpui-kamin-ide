import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { BrowserWindow } from '@kaminide/host-compat'
import type { ConnectionConfig } from '../../shared/types'
import { initBridgeState } from '../bridge-state'
import { ConnectionManager } from '../ws/connection-manager'
import { McpServerManager } from './manager'
import { mcpStartup } from './startup'

vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('./permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../hooks/emit-bridge-event', () => ({ emitBridgeHookEvent: vi.fn() }))
vi.mock('./oauth-store', () => ({ migrateLegacyStore: () => 0, loadTokens: () => null }))

const window = { isDestroyed: () => false, webContents: { send: () => {} } } as unknown as BrowserWindow
let home: string
let manager: McpServerManager
let startup: ReturnType<typeof mcpStartup>
let client: ConnectionManager | undefined
const calls: string[] = []
const firstStatus = new Map<string, number>()
const pendingHandshakes = new Map<string, Promise<void>>()
async function json(file: string, value: unknown) {
  const target = path.join(home, file)
  await fs.mkdir(path.dirname(target), { recursive: true })
  await fs.writeFile(target, JSON.stringify(value))
}

beforeEach(async () => {
  home = await fs.mkdtemp(path.join(os.tmpdir(), 'bridge-mcp-cold-'))
  vi.spyOn(os, 'homedir').mockReturnValue(home)
  initBridgeState(path.join(home, 'state'))
  vi.stubEnv('MCP_FIXTURE_ORIGIN', 'https://fixture.invalid')
  vi.stubGlobal('fetch', async (url: string, options: RequestInit) => {
    expect(new URL(url).origin).toBe('https://fixture.invalid')
    const route = new URL(url).pathname.slice(1)
    const body = JSON.parse(String(options.body))
    calls.push(route + '/' + body.method)
    if (body.method === 'initialize') await pendingHandshakes.get(route)
    const first = firstStatus.get(route)
    if (first) {
      firstStatus.delete(route)
      return new Response('fixture', { status: first })
    }
    let result: Record<string, unknown>
    switch (body.method) {
      case 'initialize': result = { capabilities: { tools: {} } }; break
      case 'notifications/initialized': return new Response(null, { status: 202 })
      case 'tools/list': result = { tools: [{ name: route, description: 'fixture', inputSchema: { type: 'object' } }] }; break
      case 'tools/call':
        expect(body.params.name).toBe(route)
        result = { content: [{ type: 'text', text: route + '-ok' }] }; break
      default: throw new Error('Unexpected fixture RPC: ' + body.method)
    }
    return Response.json({ jsonrpc: '2.0', id: body.id, result })
  })
  manager = new McpServerManager(window)
  startup = mcpStartup(() => manager.initAll())
  ConnectionManager.whenExternalToolsReady = startup.beforeAdmission
  ConnectionManager.getExternalToolSchemas = () => manager.getExternalToolSchemas()
})
afterEach(async () => {
  client?.disconnect({ endSession: true })
  client = undefined
  startup.dispose()
  manager.dispose()
  ConnectionManager.whenExternalToolsReady = null
  ConnectionManager.getExternalToolSchemas = null
  calls.length = 0
  firstStatus.clear()
  pendingHandshakes.clear()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  vi.unstubAllEnvs()
  vi.useRealTimers()
  await fs.rm(home, { recursive: true, force: true })
})

describe('cold MCP startup', () => {
  it.each(['claude', 'codex'] as const)('includes enabled user and plugin tools in the first %s session:create', async engine => {
    // A locally installed stdio connector exercises actual child startup
    // beside the two deterministic HTTP fixtures. It never opens a network socket.
    const executable = path.join(home, 'fixture.cjs')
    await fs.writeFile(executable, [
      "const lines = require('node:readline').createInterface({ input: process.stdin });",
      "lines.on('line', line => {",
      "  const request = JSON.parse(line);",
      "  if (request.id === undefined) return;",
      "  const replies = { initialize: { capabilities: { tools: {} } },",
      "    'tools/list': { tools: [{ name: 'local', inputSchema: { type: 'object' } }] },",
      "    'tools/call': { content: [{ type: 'text', text: 'local-ok' }] } };",
      "  console.log(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: replies[request.method] || {} }));",
      "});",
    ].join('\n'))
    await json('.claude.json', { mcpServers: {
      local: { type: 'stdio', command: process.execPath, args: [executable] },
      user: { type: 'http', url: '${MCP_FIXTURE_ORIGIN}/user' },
      disabled: { type: 'http', url: '${MCP_FIXTURE_ORIGIN}/hidden', disabled: true },
    } })
    await json('.claude/plugins/installed_plugins.json', { version: 2, plugins: {
      'sample@fixture': [{ scope: 'user', installPath: path.join(home, 'installed') }],
    } })
    await json('installed/.mcp.json', { mcpServers: { lookup: { type: 'http', url: '${MCP_FIXTURE_ORIGIN}/plugin' } } })
    const frames: Array<{ type: string; engine: string; externalTools: { name: string }[] }> = []
    const socket = { readyState: 1, bufferedAmount: 0, send: (f: string) => frames.push(JSON.parse(f)),
      removeAllListeners: vi.fn(), on: vi.fn(), close: vi.fn(), terminate: vi.fn() }
    client = new ConnectionManager({ serverUrl: 'ws://fixture.invalid', token: 'fixture', engine } as ConnectionConfig, window, 'tab')
    const connection = client as any
    connection.ws = socket
    await connection.startSession(socket)
    expect(frames[0]).toMatchObject({ type: 'session:create', engine })
    const names = frames[0]!.externalTools.map(tool => tool.name).sort()
    expect(names).toEqual(['local', 'plugin_sample_lookup__plugin', 'user'])
    const results = await Promise.all(names.map(name => manager.callTool(name, {})))
    expect(results.map(result => result.content[0]?.text)).toEqual(['local-ok', 'plugin-ok', 'user-ok'])
    expect(calls.some(call => call.startsWith('hidden/'))).toBe(false)
  })

  it('publishes a slow connector after admission and keeps the ready connector callable', async () => {
    await json('.claude.json', { mcpServers: {
      first: { type: 'http', url: 'https://fixture.invalid/first' },
      delayed: { type: 'http', url: 'https://fixture.invalid/delayed' },
    } })
    let finish!: () => void
    pendingHandshakes.set('delayed', new Promise<void>(resolve => { finish = resolve }))
    const snapshots: string[][] = []
    manager.onToolsChanged = () => snapshots.push(manager.getExternalToolSchemas().map(t => t.name).sort())
    try {
      await startup.beforeAdmission()
      expect(manager.getExternalToolSchemas().map(t => t.name)).toEqual(['first'])
      expect((await manager.callTool('first', {})).content[0]?.text).toBe('first-ok')
    } finally {
      finish()
    }
    await vi.waitFor(() => expect(snapshots.at(-1)).toEqual(['delayed', 'first']))
    expect((await manager.callTool('delayed', {})).content[0]?.text).toBe('delayed-ok')
  })

  it('recovers a temporary HTTP failure without Test and leaves access refusals idle', async () => {
    await json('.claude.json', { mcpServers: {
      slow: { type: 'http', url: 'https://fixture.invalid/slow' },
      denied: { type: 'http', url: 'https://fixture.invalid/denied' },
    } })
    firstStatus.set('slow', 503)
    firstStatus.set('denied', 403)
    await manager.initAll()
    expect(manager.getExternalToolSchemas()).toEqual([])
    await vi.waitFor(() => expect(manager.getExternalToolSchemas().map(t => t.name)).toEqual(['slow']), { timeout: 4500 })
    expect((await manager.callTool('slow', {})).content[0]?.text).toBe('slow-ok')
    expect(calls.filter(call => call === 'denied/initialize')).toHaveLength(1)
  })

  it('uses one deadline for simultaneous chats and lets a slow initialization finish afterward', async () => {
    vi.useFakeTimers()
    let complete!: () => void
    const init = vi.fn(() => new Promise<void>(resolve => { complete = resolve }))
    const gate = mcpStartup(init)
    const first = gate.beforeAdmission()
    expect(gate.beforeAdmission()).toBe(first)
    const admitted = vi.fn()
    void first.then(admitted)
    await vi.advanceTimersByTimeAsync(1499)
    expect(admitted).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(admitted).toHaveBeenCalledOnce()
    await gate.beforeAdmission()
    complete()
    expect(init).toHaveBeenCalledOnce()
    gate.dispose()
  })
})
