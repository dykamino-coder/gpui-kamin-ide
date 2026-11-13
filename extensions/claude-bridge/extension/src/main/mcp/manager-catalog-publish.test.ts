import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { BrowserWindow } from '@kaminide/host-compat'
import type { ExternalMcpServerConfig } from '../../shared/types'
import { McpServerManager } from './manager'

// INC-2026-0042 (b): cache-only catalog changes must reach onToolsChanged, the
// callback BridgeHost uses to send `mcp:register-external-tools` to live tabs.
const dormant: ExternalMcpServerConfig = {
  id: 'dormant', name: 'dormant', type: 'stdio', command: 'never-spawn', enabled: true,
  autoDiscovered: true, sourceType: 'plugin', pluginName: 'fixture-plugin',
}
const cold: ExternalMcpServerConfig = { ...dormant, id: 'cold', name: 'cold', command: 'cold-spawn' }
const fingerprint = (c: ExternalMcpServerConfig) =>
  JSON.stringify([c.type, c.command ?? null, c.args ?? null, null, null, null, null])
const cachedTool = { name: 'lookup', description: 'cached lookup', inputSchema: { type: 'object', properties: {} } }

const hoisted = vi.hoisted(() => ({ cache: {} as Record<string, unknown> }))
vi.mock('./state-persistence', () => ({
  getMcpStore: () => ({ get: () => [], set: () => {} }),
  loadToolSchemaCache: () => hoisted.cache,
  saveToolSchemaCache: vi.fn(),
  mutateClaudeJson: vi.fn(async () => {}),
  configToClaudeJsonEntry: (c: unknown) => c,
}))
vi.mock('./oauth-store', () => ({ migrateLegacyStore: () => 0, deleteTokens: vi.fn(), loadTokens: () => null }))
vi.mock('../hooks/emit-bridge-event', () => ({ emitBridgeHookEvent: vi.fn() }))
vi.mock('./discovery', () => ({
  discoverPluginConfigs: vi.fn(async () => []),
  discoverFromClaudeJsonAsync: vi.fn(async () => []),
  discoverProjectConfigs: vi.fn(async () => []),
  resolveEnvVars: (value: unknown) => value,
}))
vi.mock('./transports/stdio', () => ({ connectStdio: vi.fn(), stdioWorkingDirectory: () => '/fixture' }))

import { discoverPluginConfigs } from './discovery'
import { connectStdio } from './transports/stdio'

const managers: McpServerManager[] = []
function manager() {
  const instance = new McpServerManager({ isDestroyed: () => false, webContents: { send: vi.fn() } } as unknown as BrowserWindow)
  managers.push(instance)
  const published = vi.fn(() => instance.getExternalToolSchemas().map(t => `${t.name}:${t.description}`))
  instance.onToolsChanged = published
  return { instance, published }
}
const lastCatalog = (spy: ReturnType<typeof manager>['published']) => spy.mock.results.at(-1)?.value

beforeEach(() => {
  vi.useFakeTimers()
  hoisted.cache = { [fingerprint(dormant)]: [{ ...cachedTool }] }
})
afterEach(() => {
  for (const instance of managers.splice(0)) instance.dispose()
  vi.useRealTimers()
  vi.clearAllMocks()
})

describe('MCP catalog publication after cache-only changes', () => {
  it('publishes warm-cache tools found by discovery without spawning the connector', async () => {
    vi.mocked(discoverPluginConfigs).mockResolvedValue([{ ...dormant }])
    const { instance, published } = manager()
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    expect(connectStdio).not.toHaveBeenCalled()
    expect(published).toHaveBeenCalledOnce()
    expect(lastCatalog(published)).toEqual(['plugin_fixture-plugin_dormant__lookup:cached lookup'])
  })

  it('publishes an empty catalog when a dormant connector is removed', async () => {
    vi.mocked(discoverPluginConfigs).mockResolvedValue([{ ...dormant }])
    const { instance, published } = manager()
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    published.mockClear()
    instance.removeServer('dormant')
    await vi.advanceTimersByTimeAsync(250)
    expect(published).toHaveBeenCalledOnce()
    expect(lastCatalog(published)).toEqual([])
  })

  it('does not republish an unchanged catalog on repeated discovery', async () => {
    vi.mocked(discoverPluginConfigs).mockResolvedValue([{ ...dormant }])
    const { instance, published } = manager()
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    published.mockClear()
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    expect(published).not.toHaveBeenCalled()
  })

  it('publishes same-count schema changes and discovery removal', async () => {
    vi.mocked(discoverPluginConfigs).mockResolvedValue([{ ...dormant }])
    const { instance, published } = manager()
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    published.mockClear()
    const schemas = hoisted.cache[fingerprint(dormant)] as typeof cachedTool[]
    schemas[0] = { ...cachedTool, description: 'updated lookup' }
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    expect(published).toHaveBeenCalledOnce()
    expect(lastCatalog(published)).toEqual(['plugin_fixture-plugin_dormant__lookup:updated lookup'])
    published.mockClear()
    vi.mocked(discoverPluginConfigs).mockResolvedValue([])
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    expect(published).toHaveBeenCalledOnce()
    expect(lastCatalog(published)).toEqual([])
  })

  it('does not defer explicit removal behind an unrelated pending connection', async () => {
    vi.mocked(discoverPluginConfigs).mockResolvedValue([{ ...dormant }])
    const { instance, published } = manager()
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    published.mockClear()
    let finish!: () => void
    vi.mocked(discoverPluginConfigs).mockResolvedValue([{ ...dormant }, { ...cold }])
    vi.mocked(connectStdio).mockImplementationOnce(() => new Promise<void>(done => { finish = done }))
    const pending = instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    instance.removeServer('dormant')
    expect(published).toHaveBeenCalledOnce()
    expect(lastCatalog(published)).toEqual([])
    await vi.advanceTimersByTimeAsync(250)
    expect(published).toHaveBeenCalledOnce()
    finish()
    await pending
  })

  it('withholds the intermediate catalog while a connector is connecting', async () => {
    vi.mocked(discoverPluginConfigs).mockResolvedValue([{ ...dormant }, { ...cold }])
    let finish!: () => void
    vi.mocked(connectStdio).mockImplementationOnce(async (context, id) => {
      await new Promise<void>(done => { finish = done })
      const state = context.servers.get(id)!
      state.tools = ['fresh']
      state.toolSchemas = new Map([['fresh', { name: 'fresh', description: 'live fresh', inputSchema: { type: 'object' } }]])
    })
    const { instance, published } = manager()
    const pending = instance.initAll()
    await vi.advanceTimersByTimeAsync(250)
    expect(published).not.toHaveBeenCalled()
    finish()
    await pending
    await vi.advanceTimersByTimeAsync(250)
    expect(published).toHaveBeenCalledOnce()
    expect(lastCatalog(published)).toEqual([
      'plugin_fixture-plugin_cold__fresh:live fresh',
      'plugin_fixture-plugin_dormant__lookup:cached lookup',
    ])
  })
})
