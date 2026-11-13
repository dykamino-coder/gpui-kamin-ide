import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { BridgeHost } from './bridge-host'

const fixture = vi.hoisted(() => ({
  files: new Map<string, string>(),
  active: undefined as any,
  sessions: [] as any[],
}))
vi.mock('fs', () => ({ default: {
  appendFileSync: vi.fn(), mkdirSync: vi.fn(),
  readFileSync: (file: string) => { if (!fixture.files.has(file)) throw Error('ENOENT'); return fixture.files.get(file) },
  writeFileSync: (file: string, data: string) => fixture.files.set(file, data),
  renameSync: (from: string, to: string) => { fixture.files.set(to, fixture.files.get(from)!); fixture.files.delete(from) },
} }))
vi.mock('kaminide', () => ({ sessions: {
  get active() { return fixture.active }, get all() { return fixture.sessions },
  projects: [{ id: 'p', folderPath: '/project' }],
  updateSession: vi.fn(), setActiveSession: vi.fn(),
  onDidChangeActiveSession: () => ({ dispose() {} }),
  onDidChangeSessions: () => ({ dispose() {} }),
} }))
vi.mock('@kaminide/host-compat', () => ({ ipcMain: { on: vi.fn() } }))
vi.mock('./main/ws/connection-manager', () => ({ ConnectionManager: class {
  status = 'disconnected'
  constructor(private config: { engine?: string }) {}
  connect = vi.fn(() => { this.status = 'connecting' })
  disconnect = vi.fn()
  dispose = vi.fn()
  setRendererMuted = vi.fn()
  getState = () => ({ status: this.status, authority: 'synthetic', revision: 1 })
  getExtendedState = () => ({ status: this.status })
  getEngine = () => this.config.engine ?? 'claude'
  replayJsonlToRenderer = vi.fn()
} }))
vi.mock('./main/sync/sync-client', () => ({ syncUserData: vi.fn(), syncProjectData: vi.fn(), resetSyncTimers: vi.fn() }))
import { TabManager } from './main/tab-manager'
import { syncUserData } from './main/sync/sync-client'
import { SessionsBridge } from './sessions-bridge'

const cfg = { serverUrl: 'ws://synthetic.invalid', token: 'synthetic-not-a-credential' }
const bridges: SessionsBridge[] = []
function child(generation: number, token = cfg.token) {
  vi.stubEnv('KAMIN_EXTHOST_GENERATION', String(generation))
  const send = vi.fn()
  const tm = new TabManager({ isDestroyed: () => false, webContents: { send } } as any)
  const host = { getTabManager: () => tm, getConfigStore: () => ({ get: () => ({ ...cfg, token }) }) }
  const bridge = new SessionsBridge(host as unknown as BridgeHost, vi.fn(), '/synthetic-storage')
  bridge.install()
  bridges.push(bridge)
  return { tm, bridge, send }
}
beforeEach(() => {
  vi.useFakeTimers()
  fixture.files.clear()
  fixture.sessions = [{ id: 'native-a', projectId: 'p', name: 'A', open: true, metadata: { bridge: { conversationId: 'conversation-a', engine: 'claude' } } }]
  fixture.active = fixture.sessions[0]
  vi.mocked(syncUserData).mockClear()
})
afterEach(() => {
  for (const bridge of bridges.splice(0)) bridge.dispose()
  vi.clearAllTimers(); vi.useRealTimers(); vi.unstubAllEnvs()
})

describe('BR-04 native child respawn', () => {
  it('restores stable IDs and the selected tab without native active state or another data sync', () => {
    const first = child(1)
    const nativeId = first.tm.getActiveTabId()!
    const selected = first.tm.resumeSession({ ...cfg, cwd: '/other', conversationId: 'conversation-b', engine: 'codex' })
    const identities = first.tm.listTabs().map(({ id, label, createdAt }) => ({ id, label, createdAt }))
    first.bridge.dispose() // child dies; the parent and retained frames survive
    fixture.active = undefined
    vi.mocked(syncUserData).mockClear()
    const second = child(2)
    expect(second.tm.listTabs().map(({ id, label, createdAt }) => ({ id, label, createdAt }))).toEqual(identities)
    expect(second.tm.getActiveTabId()).toBe(selected)
    expect(second.tm.getConnection(selected)!.connect).toHaveBeenCalledTimes(1)
    expect(second.tm.getConnection(nativeId)!.connect).not.toHaveBeenCalled()
    expect(syncUserData).not.toHaveBeenCalled()
    expect(second.tm.getTab(selected)?.conversationId).toBe('conversation-b')
    expect(second.tm.getConnection(selected)?.getEngine()).toBe('codex')
    // Resolving Chat and Agents after the respawn explicitly rebinds and replays.
    second.bridge.resyncActive()
    vi.advanceTimersByTime(300)
    expect(second.send).toHaveBeenCalledWith('tab:list-changed', expect.arrayContaining([
      expect.objectContaining({ id: selected, status: 'connecting' }),
    ]))
    expect(second.tm.getConnection(selected)!.replayJsonlToRenderer).toHaveBeenCalledTimes(1)
    // A second death must preserve the same identity again.
    second.bridge.dispose()
    const third = child(3)
    expect(third.tm.getActiveTabId()).toBe(selected)
    expect(third.tm.listTabs().map(t => t.id)).toEqual([nativeId, selected])
  })

  it('reattaches the native session mapping and honours its current selection', () => {
    const first = child(1), selected = first.tm.getActiveTabId()!
    first.bridge.dispose()
    const second = child(2)
    expect(second.tm.getActiveTabId()).toBe(selected)
    expect(second.tm.size).toBe(1)
    expect(second.tm.getConnection(selected)!.connect).toHaveBeenCalledTimes(1)
  })

  it('preserves a direct Bridge selection when the native sidebar selection has not changed', () => {
    const first = child(1)
    const selected = first.tm.resumeSession({ ...cfg, conversationId: 'direct-conversation' })
    first.bridge.dispose()
    expect(child(2).tm.getActiveTabId()).toBe(selected)
  })

  it('honours a native selection changed while the child was down', () => {
    const first = child(1)
    const nativeB = { ...fixture.sessions[0], id: 'native-b', metadata: { bridge: { conversationId: 'conversation-b' } } }
    fixture.sessions.push(nativeB)
    vi.advanceTimersByTime(4000)
    const selected = first.tm.listTabs().find(t => t.conversationId === 'conversation-b')!.id
    first.bridge.dispose()
    fixture.active = nativeB
    const second = child(2)
    expect(second.tm.getActiveTabId()).toBe(selected)
    expect(second.tm.size).toBe(2)
  })

  it('does not revive a disconnected native session', () => {
    const first = child(1), closed = first.tm.getActiveTabId()!
    first.bridge.dispose()
    fixture.active = undefined
    fixture.sessions[0].open = false
    const second = child(2)
    expect(second.tm.getTab(closed)).toBeUndefined()
    expect(second.tm.getActiveTabId()).toBeNull()
  })

  it('does not restore another credential scope or a cold app start', () => {
    const first = child(1), oldId = first.tm.getActiveTabId()
    first.bridge.dispose()
    const otherScope = child(2, 'other-synthetic-token')
    expect(otherScope.tm.getActiveTabId()).not.toBe(oldId)
    otherScope.bridge.dispose()
    const cold = child(1)
    expect(cold.tm.getActiveTabId()).not.toBe(oldId)
    expect([...fixture.files.values()].join('')).not.toContain(cfg.token)
  })
})
