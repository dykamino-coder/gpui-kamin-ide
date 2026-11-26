import { afterEach, describe, expect, it, vi } from 'vitest'
import os from 'node:os'
import type { BrowserWindow } from '@kaminide/host-compat'
import type { ExternalMcpServerConfig } from '../../shared/types'
import type { TransportContext } from './transports/context'
import { McpServerManager } from './manager'

// Keep operator files and real connector processes out of this lifecycle test.
vi.mock('./state-persistence', () => ({
  getMcpStore: () => ({ get: () => [] }),
  loadToolSchemaCache: () => ({}),
  saveToolSchemaCache: vi.fn(),
}))
vi.mock('./oauth-store', () => ({ migrateLegacyStore: () => 0 }))
vi.mock('./discovery', () => ({
  discoverPluginConfigs: vi.fn(async () => []),
  discoverFromClaudeJsonAsync: vi.fn(async () => []),
  discoverProjectConfigs: vi.fn(async () => []),
  resolveEnvVars: (value: unknown) => value,
}))
vi.mock('./transports/stdio', async (importOriginal) => ({
  ...await importOriginal<typeof import('./transports/stdio')>(),
  connectStdio: vi.fn(),
}))

import { discoverFromClaudeJsonAsync } from './discovery'
import { connectStdio } from './transports/stdio'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}
const config: ExternalMcpServerConfig = {
  id: 'fixture', name: 'fixture', type: 'stdio', command: 'never-spawn', enabled: true,
}
const managers: McpServerManager[] = []
function manager() {
  const send = vi.fn()
  const instance = new McpServerManager({ isDestroyed: () => false, webContents: { send } } as unknown as BrowserWindow)
  managers.push(instance)
  return { instance, send }
}
afterEach(() => { for (const instance of managers.splice(0)) instance.dispose(); vi.clearAllMocks() })

describe('MCP manager final shutdown', () => {
  it('ignores discovery that finishes after disposal and cannot reinitialize', async () => {
    const discovery = deferred<ExternalMcpServerConfig[]>()
    const entered = deferred<void>()
    vi.mocked(discoverFromClaudeJsonAsync).mockImplementationOnce(() => {
      entered.resolve()
      return discovery.promise
    })
    const { instance, send } = manager()
    const pending = instance.initAll()
    await entered.promise
    instance.dispose()
    instance.dispose()
    discovery.resolve([config])
    await pending
    await instance.initAll()
    expect(instance.getServers()).toEqual([])
    expect(connectStdio).not.toHaveBeenCalled()
    expect(send).not.toHaveBeenCalled()
  })

  it('tears down a transport acquired after disposal without publishing or reconnecting', async () => {
    vi.mocked(discoverFromClaudeJsonAsync).mockResolvedValueOnce([config])
    const connecting = deferred<void>()
    const entered = deferred<TransportContext>()
    const kill = vi.fn()
    vi.mocked(connectStdio).mockImplementationOnce(async (context, id) => {
      entered.resolve(context)
      await connecting.promise
      context.servers.get(id)!.process = { kill } as never
    })
    const { instance, send } = manager()
    const changed = vi.fn()
    instance.onToolsChanged = changed
    const pending = instance.initAll()
    const context = await entered.promise
    instance.dispose()
    send.mockClear()
    connecting.resolve()
    await pending
    context.onUnexpectedDisconnect?.('fixture', 'late exit')
    await instance.initAll()
    expect(kill).toHaveBeenCalledOnce()
    expect(instance.getServers()[0].status).toBe('disconnected')
    expect(instance.servers.get('fixture')!.reconnectTimer).toBeUndefined()
    expect(connectStdio).toHaveBeenCalledOnce()
    expect(changed).not.toHaveBeenCalled()
    expect(send).not.toHaveBeenCalled()
  })
})

it('keeps a permanent startup diagnosis and retries only on explicit Test or changed config', async () => {
  const installed = { ...config, autoDiscovered: true }
  vi.mocked(discoverFromClaudeJsonAsync).mockResolvedValue([installed])
  vi.mocked(connectStdio).mockImplementation(async (context, id) => {
    context.servers.get(id)!.permanentFailure = 'MODULE_NOT_FOUND: missing fixture dependency'
    throw new Error('Process exited early (code=1)')
  })
  const { instance } = manager()
  await instance.initAll()
  expect(instance.getServers()[0]?.error).toContain('missing fixture dependency')
  await instance.initAll()
  expect(connectStdio).toHaveBeenCalledTimes(1)
  expect(instance.getServers()[0]?.error).toContain('missing fixture dependency')
  expect((await instance.testServer(installed.id)).error).toContain('missing fixture dependency')
  expect(connectStdio).toHaveBeenCalledTimes(2)
  vi.mocked(discoverFromClaudeJsonAsync).mockResolvedValue([{ ...installed, command: 'changed' }])
  await instance.initAll()
  expect(connectStdio).toHaveBeenCalledTimes(3)
})

it('reports the missing dependency from a real stdio process after an unchanged rescan', async () => {
  const transport = await vi.importActual<typeof import('./transports/stdio')>('./transports/stdio')
  vi.mocked(connectStdio).mockImplementation(transport.connectStdio)
  const fixture = { ...config, autoDiscovered: true, workingDirectory: os.tmpdir(),
    command: process.execPath, args: ['-e', "require('missing-synthetic-bridge-dependency')"] }
  vi.mocked(discoverFromClaudeJsonAsync).mockResolvedValue([fixture])
  const { instance } = manager()
  await instance.initAll()
  expect(instance.getServers()[0]?.error).toContain('missing-synthetic-bridge-dependency')
  await instance.initAll()
  expect(connectStdio).toHaveBeenCalledTimes(1)
  expect(instance.getServers()[0]?.status).toBe('error')
})

it('exhausts ordinary exit retries with bounded backoff and leaves the server idle', async () => {
  vi.useFakeTimers()
  const fixture = { ...config, autoDiscovered: true }
  vi.mocked(discoverFromClaudeJsonAsync).mockResolvedValue([fixture])
  vi.mocked(connectStdio).mockImplementation(async (context, id) => {
    context.onUnexpectedDisconnect?.(id, 'process exited code=1')
    throw new Error('Process exited early (code=1)')
  })
  const { instance } = manager()
  try {
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(1999)
    expect(connectStdio).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(1)
    expect(connectStdio).toHaveBeenCalledTimes(2)
    await vi.advanceTimersByTimeAsync(15 * 60_000)
    const attempts = vi.mocked(connectStdio).mock.calls.length
    expect(attempts).toBe(11)
    await instance.initAll()
    await vi.advanceTimersByTimeAsync(15 * 60_000)
    expect(connectStdio).toHaveBeenCalledTimes(attempts)
    expect(instance.getServers()[0]?.status).toBe('error')
  } finally { vi.useRealTimers() }
})
