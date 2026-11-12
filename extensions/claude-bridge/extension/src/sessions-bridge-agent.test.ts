import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { BridgeHost } from './bridge-host'

// A KaminIDE session's chat runs on the engine stored in metadata.bridge.engine;
// a brand-new session takes a pending "open a GPT chat" pick or the main agent.

const k = vi.hoisted(() => ({
  active: undefined as { id: string; projectId: string } | undefined,
  onCreate: null as null | ((s: { id: string; projectId: string; metadata?: Record<string, unknown> }) => void),
  updateSession: vi.fn(async () => undefined),
  setActiveSession: vi.fn(),
  createSession: vi.fn(),
}))
vi.mock('kaminide', () => ({
  sessions: {
    get active() { return k.active },
    projects: [{ id: 'p1', folderPath: 'C:/work' }],
    updateSession: k.updateSession,
    setActiveSession: k.setActiveSession,
    createSession: k.createSession,
  },
}))
vi.mock('@kaminide/host-compat', () => ({ ipcMain: {} }))
vi.mock('./main/ws/connection-manager', () => ({ ConnectionManager: {} }))
import { SessionsBridge } from './sessions-bridge'
import { agentForNewChat, bridgeMetaFor, readBridgeMeta } from './agent-session'

function fixture(defaultEngine: 'claude' | 'codex' = 'claude') {
  const tm = {
    getTab: vi.fn(() => undefined),
    switchTab: vi.fn(),
    createTab: vi.fn(() => 'tab-new'),
    resumeSession: vi.fn(() => 'tab-resumed'),
  }
  const host = {
    getTabManager: () => tm,
    getConfigStore: () => ({ get: () => ({ serverUrl: 'ws://synthetic', token: 'synthetic' }), getDefaultEngine: () => defaultEngine }),
  }
  const bridge = new SessionsBridge(host as unknown as BridgeHost, vi.fn())
  const onActive = (s: unknown) => (bridge as unknown as { onActive: (s: unknown) => void }).onActive(s)
  return { bridge, tm, onActive }
}

beforeEach(() => {
  k.active = { id: 'current', projectId: 'p1' }
  k.updateSession.mockClear()
  k.setActiveSession.mockClear()
  k.createSession.mockReset()
})

describe('sessions bridge: chat engine per KaminIDE session', () => {
  it('a GPT pick opens a new session whose chat starts on codex with that model', async () => {
    const f = fixture('claude')
    // The host activates the new session before its metadata is written.
    k.createSession.mockImplementation(async (opts: { projectId?: string }) => {
      const s = { id: 'fresh', projectId: opts.projectId ?? 'p1', lastOpened: 0, createdAt: 0, name: 'New' }
      f.onActive(s)
      return s
    })
    await f.bridge.openAgentChat('codex', 'gpt-5.5')
    expect(k.createSession).toHaveBeenCalledWith({ projectId: 'p1' })
    expect(f.tm.createTab).toHaveBeenCalledWith({ serverUrl: 'ws://synthetic', token: 'synthetic', cwd: 'C:/work', engine: 'codex', model: 'gpt-5.5' })
    expect(k.updateSession).toHaveBeenCalledWith('fresh', { metadata: { bridge: { conversationId: null, cwd: null, engine: 'codex', model: 'gpt-5.5' } } })
    expect(k.setActiveSession).toHaveBeenCalledWith('fresh')
  })

  it('ignores an unknown engine', async () => {
    const f = fixture()
    await f.bridge.openAgentChat('gemini', 'x')
    expect(k.createSession).not.toHaveBeenCalled()
  })

  it('a new session without a stored engine opens on the main agent', () => {
    const f = fixture('codex')
    f.onActive({ id: 's1', projectId: 'p1', metadata: {} })
    expect(f.tm.createTab).toHaveBeenCalledWith({ serverUrl: 'ws://synthetic', token: 'synthetic', cwd: 'C:/work', engine: 'codex' })
  })

  it('a session with a conversation resumes with its stored engine as a hint', () => {
    const f = fixture('claude')
    f.onActive({ id: 's2', projectId: 'p1', name: 'GPT chat', metadata: { bridge: { conversationId: 'conv-1', engine: 'codex' } } })
    expect(f.tm.resumeSession).toHaveBeenCalledWith(expect.objectContaining({ conversationId: 'conv-1', engine: 'codex' }))
    expect(f.tm.createTab).not.toHaveBeenCalled()
  })
})

describe('agent-session metadata helpers', () => {
  it('reads only well-formed fields', () => {
    expect(readBridgeMeta({ bridge: { conversationId: 'c', engine: 'codex', model: 'gpt-5.5' } }))
      .toEqual({ conversationId: 'c', cwd: null, engine: 'codex', model: 'gpt-5.5' })
    expect(readBridgeMeta({ bridge: { engine: 'gemini', model: 5 } })).toEqual({ conversationId: null, cwd: null })
    expect(readBridgeMeta({})).toBeNull()
  })

  it('prefers the stored engine, then the pending pick, then the main agent', () => {
    expect(agentForNewChat({ engine: 'claude' }, { engine: 'codex' }, 'codex')).toEqual({ engine: 'claude' })
    expect(agentForNewChat(null, { engine: 'codex', model: 'gpt-5.5' }, 'claude')).toEqual({ engine: 'codex', model: 'gpt-5.5' })
    expect(agentForNewChat(null, null, 'codex')).toEqual({ engine: 'codex' })
  })

  it('keeps the engine when the conversation id is written back', () => {
    expect(bridgeMetaFor('c', 'C:/work', 'codex')).toEqual({ conversationId: 'c', cwd: 'C:/work', engine: 'codex' })
    expect(bridgeMetaFor(null, null, undefined)).toEqual({ conversationId: null, cwd: null })
  })
})
