import fs from 'node:fs'
import path from 'node:path'
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest'

const fixture = vi.hoisted(() => ({
  home: `${process.env.KAMIN_TEST_EVIDENCE_ROOT ?? process.env.TEMP ?? '/tmp'}/bridge-profile-fixture-${process.pid}`,
  handlers: new Map<string, (...args: any[]) => any>(),
  pluginRoot: '',
}))
vi.mock('os', async importOriginal => {
  const original = await importOriginal<typeof import('os')>()
  return { ...original, default: { ...original, homedir: () => fixture.home }, homedir: () => fixture.home }
})
vi.mock('@kaminide/host-compat', () => ({ ipcMain: { handle: (name: string, handler: (...args: any[]) => any) => fixture.handlers.set(name, handler) } }))
vi.mock('./plugin-helpers', async importOriginal => ({
  ...await importOriginal<typeof import('./plugin-helpers')>(),
  listEnabledInstalledPlugins: async () => [{ key: 'sample@local', name: 'sample', marketplace: 'local', installPath: fixture.pluginRoot }],
  readEffectivePluginManifest: async () => ({ hooks: { PreToolUse: [{ matcher: 'Read', hooks: [{ type: 'command', command: 'synthetic-guard' }] }] } }),
}))
vi.mock('./plugin-monitors', () => ({ buildMonitorTriggerHooks: async () => ({}) }))
vi.mock('./sync/sync-client', () => ({ resetSyncTimers: vi.fn(), syncUserData: vi.fn() }))

import { initBridgeState, bridgeStatePath } from './bridge-state'
import { saveTabsState, loadTabsState, type PersistedTab } from './tabs-state-store'
import { getMcpStore, loadToolSchemaCache, saveToolSchemaCache } from './mcp/state-persistence'
import { registerHooksIPC } from './ipc/hooks'
import { getPendingHookApproval } from './ipc/plugins/handlers-install'
import { buildPluginSnapshots, hashPluginHook } from './sync/plugin-snapshot'
import { ConfigStore } from './config/store'

const legacy = path.join(fixture.home, '.claude', 'open-claude-bridge')
const profile = (name: string) => path.join(fixture.home, 'profiles', name, 'globalStorage', 'bridge')
const tab = (id: string): PersistedTab => ({ id, title: id, cwd: '/synthetic', pinned: false, order: 0 })
const invoke = (name: string, ...args: unknown[]) => fixture.handlers.get(name)!({}, ...args)
const read = (file: string) => JSON.parse(fs.readFileSync(file, 'utf8'))

beforeEach(() => {
  fs.rmSync(fixture.home, { recursive: true, force: true })
  fs.mkdirSync(legacy, { recursive: true })
  fixture.pluginRoot = path.join(fixture.home, 'synthetic-plugin')
  fs.mkdirSync(fixture.pluginRoot, { recursive: true })
  fixture.handlers.clear()
})
afterAll(() => fs.rmSync(fixture.home, { recursive: true, force: true }))

describe('INC-2026-0111 profile-owned Bridge state', () => {
  it('keeps tabs isolated with the same home and connection token, including reopen', () => {
    initBridgeState(profile('a'))
    saveTabsState([tab('a')], 'synthetic-token')
    initBridgeState(profile('b'))
    expect(loadTabsState('synthetic-token')).toBeNull()
    saveTabsState([tab('b')], 'synthetic-token')
    initBridgeState(profile('a'))
    expect(loadTabsState('synthetic-token')).toEqual([tab('a')])
    expect(loadTabsState('different-token')).toBeNull()
    expect(fs.existsSync(path.join(legacy, 'tabs-state.json'))).toBe(false)
    expect(read(path.join(profile('b'), 'tabs-state.json')).tabs).toEqual([tab('b')])
  })

  it('isolates both MCP stores and does not reuse a singleton from another profile', () => {
    initBridgeState(profile('a'))
    getMcpStore().set('mcpServers', [{ id: 'a', name: 'a', type: 'stdio', command: 'synthetic-a' } as any])
    saveToolSchemaCache({ a: [{ name: 'tool-a', description: '', inputSchema: {} }] })
    initBridgeState(profile('b'))
    expect(getMcpStore().get('mcpServers')).toEqual([])
    expect(loadToolSchemaCache()).toEqual({})
    getMcpStore().set('mcpServers', [])
    saveToolSchemaCache({ b: [] })
    initBridgeState(profile('a'))
    expect(getMcpStore().get('mcpServers')[0]).toMatchObject({ id: 'a' })
    expect(loadToolSchemaCache()).toHaveProperty('a')
    expect(fs.readdirSync(legacy)).toEqual([])
  })

  it('uses one profile approval store for IPC, install review and sync snapshots', async () => {
    const hash = hashPluginHook('PreToolUse', 'Read', { type: 'command', command: 'synthetic-guard' })
    initBridgeState(profile('a'))
    registerHooksIPC({ configStore: new ConfigStore(profile('a')) })
    expect(await invoke('hooks:set-plugin-approval', 'sample@local', [hash])).toMatchObject({ ok: true })
    expect(read(bridgeStatePath('plugin-hook-approvals.json'))['sample@local']).toContain(hash)
    expect(await getPendingHookApproval('sample@local', fixture.pluginRoot)).toBeNull()
    expect((await buildPluginSnapshots())['sample@local'].hooks.PreToolUse).toHaveLength(1)
    initBridgeState(profile('b'))
    expect(await invoke('hooks:get-plugin-approval', 'sample@local')).toEqual({ approved: false, hashes: [] })
    expect(await getPendingHookApproval('sample@local', fixture.pluginRoot)).not.toBeNull()
    expect((await buildPluginSnapshots())['sample@local'].hooks).toEqual({})
    expect(fs.existsSync(path.join(legacy, 'plugin-hook-approvals.json'))).toBe(false)
  })

  it('imports legacy files only once for the default profile, preserving source and existing state', () => {
    const names = ['tabs-state.json', 'plugin-hook-approvals.json', 'mcp-servers.json', 'mcp-tool-schemas.json']
    for (const name of names) fs.writeFileSync(path.join(legacy, name), JSON.stringify({ legacy: name }))
    const mtimes = names.map(name => fs.statSync(path.join(legacy, name)).mtimeMs)
    fs.writeFileSync(path.join(legacy, 'mcp-oauth-tokens.json'), 'synthetic-excluded')
    initBridgeState(profile('test'), { legacyDir: legacy })
    expect(fs.existsSync(profile('test'))).toBe(false)
    fs.mkdirSync(profile('default'), { recursive: true })
    fs.writeFileSync(path.join(profile('default'), names[0]), '{"existing":true}')
    initBridgeState(profile('default'), { isDefaultProfile: true, legacyDir: legacy })
    expect(read(bridgeStatePath(names[0]))).toEqual({ existing: true })
    for (const name of names.slice(1)) expect(read(bridgeStatePath(name))).toEqual({ legacy: name })
    expect(fs.existsSync(bridgeStatePath('mcp-oauth-tokens.json'))).toBe(false)
    expect(names.map(name => fs.statSync(path.join(legacy, name)).mtimeMs)).toEqual(mtimes)
    fs.unlinkSync(bridgeStatePath(names[1]))
    fs.writeFileSync(path.join(legacy, names[1]), '{"late":true}')
    initBridgeState(profile('default'), { isDefaultProfile: true, legacyDir: legacy })
    expect(fs.existsSync(bridgeStatePath(names[1]))).toBe(false)
  })

  it('marks an absent legacy directory as migrated rather than importing new home state later', () => {
    const absent = path.join(fixture.home, 'absent')
    initBridgeState(profile('default'), { isDefaultProfile: true, legacyDir: absent })
    fs.mkdirSync(absent)
    fs.writeFileSync(path.join(absent, 'tabs-state.json'), '{}')
    initBridgeState(profile('default'), { isDefaultProfile: true, legacyDir: absent })
    expect(fs.existsSync(bridgeStatePath('tabs-state.json'))).toBe(false)
  })
})
