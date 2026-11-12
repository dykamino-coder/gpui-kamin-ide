import { afterEach, describe, expect, it, vi } from 'vitest'

// A chat's agent (Claude / ChatGPT via codex) on the client: the engine rides
// session:create, the server's answer sticks to the tab, the GPT model the user
// picked is applied once, and the engine-level replies reach the webview.

vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/executor', () => ({ executeTool: vi.fn() }))

import { ConnectionManager } from './connection-manager'
import { handleServerMessage, type HandlerCtx } from './handle-server-message'
import type { ConnectionConfig } from '../../shared/types'
import type { BrowserWindow } from '@kaminide/host-compat'

const managers: ConnectionManager[] = []

function manager(config: Partial<ConnectionConfig>, stored?: string) {
  const sent: Array<{ channel: string; args: unknown[] }> = []
  const window = { webContents: { send: (channel: string, ...args: unknown[]) => sent.push({ channel, args }) } }
  const m = new ConnectionManager({ serverUrl: 'ws://synthetic', token: 'synthetic', ...config } as ConnectionConfig,
    window as unknown as BrowserWindow, 'tab', stored)
  managers.push(m)
  const frames: Array<Record<string, unknown>> = []
  const socket = {
    readyState: 1, bufferedAmount: 0,
    send: (frame: string) => frames.push(JSON.parse(frame)),
    removeAllListeners: vi.fn(), on: vi.fn(), close: vi.fn(), terminate: vi.fn(),
  }
  const internal = m as any
  internal.ws = socket
  return { m, internal, frames, socket, sent }
}

afterEach(() => {
  for (const m of managers.splice(0)) m.disconnect({ endSession: true })
  ConnectionManager.onAgentPreferences = null
})

describe('agent engine on the client connection', () => {
  it('sends the engine of a new chat and keeps older clients engine-less', async () => {
    const codex = manager({ engine: 'codex' })
    await codex.internal.startSession(codex.socket)
    expect(codex.frames[0]).toMatchObject({ type: 'session:create', engine: 'codex' })

    const plain = manager({})
    await plain.internal.startSession(plain.socket)
    expect(plain.frames[0]!.type).toBe('session:create')
    expect(plain.frames[0]).not.toHaveProperty('engine')
    expect(plain.m.getEngine()).toBe('claude')
  })

  it('hints the stored engine on resume', async () => {
    const { internal, frames, socket } = manager({ engine: 'codex' }, '00000000-0000-4000-8000-000000000001')
    await internal.startSession(socket)
    expect(frames[0]).toMatchObject({ type: 'session:resume', engine: 'codex' })
  })

  it('adopts the server engine and applies the picked GPT model once', () => {
    const { m, internal, frames, sent } = manager({ engine: 'codex', model: 'gpt-5.5' })
    internal.dispatchMessage({ type: 'session:created', sessionId: 's1', model: 'gpt-5.4', engine: 'codex' })
    expect(m.getEngine()).toBe('codex')
    expect(frames.filter((f) => f.type === 'session:change-model')).toEqual([{ type: 'session:change-model', model: 'gpt-5.5' }])
    const restarted = sent.find((e) => e.channel === 'session-restarted')
    expect(restarted?.args[1]).toMatchObject({ engine: 'codex', model: 'gpt-5.4' })

    // A reconnect of the same chat does not re-apply the original pick.
    internal.dispatchMessage({ type: 'session:created', sessionId: 's2', model: 'gpt-5.4', engine: 'codex' })
    expect(frames.filter((f) => f.type === 'session:change-model')).toHaveLength(1)
  })

  it('applies a Claude model picked in a GPT chat to the new Claude chat once', () => {
    const { internal, frames } = manager({ engine: 'claude', model: 'claude-sonnet-5-5' })
    internal.dispatchMessage({ type: 'session:created', sessionId: 's1', model: 'claude-opus-5-5', engine: 'claude' })
    expect(frames.filter((f) => f.type === 'session:change-model')).toEqual([
      { type: 'session:change-model', model: 'claude-sonnet-5-5' },
    ])
    internal.dispatchMessage({ type: 'session:created', sessionId: 's2', model: 'claude-opus-5-5', engine: 'claude' })
    expect(frames.filter((f) => f.type === 'session:change-model')).toHaveLength(1)
  })

  it('drops the picked model when the server opened the chat on the other engine', () => {
    // A GPT chat whose ChatGPT side is signed out runs Claude (engine fallback):
    // the GPT model id must not be sent to the Claude session, nor the reverse.
    const gpt = manager({ engine: 'codex', model: 'gpt-6-luna' })
    gpt.internal.dispatchMessage({ type: 'session:created', sessionId: 's1', model: 'claude-opus-5-5', engine: 'claude' })
    expect(gpt.m.getEngine()).toBe('claude')
    expect(gpt.frames.some((f) => f.type === 'session:change-model')).toBe(false)

    const claude = manager({ engine: 'claude', model: 'claude-sonnet-5-5' })
    claude.internal.dispatchMessage({ type: 'session:created', sessionId: 's1', model: 'gpt-6-luna', engine: 'codex' })
    expect(claude.frames.some((f) => f.type === 'session:change-model')).toBe(false)
  })

  it('does not send a model change when codex already started on the picked model', () => {
    const { internal, frames } = manager({ engine: 'codex', model: 'gpt-5.5' })
    internal.dispatchMessage({ type: 'session:created', sessionId: 's1', model: 'gpt-5.5', engine: 'codex' })
    expect(frames.some((f) => f.type === 'session:change-model')).toBe(false)
  })

  it('trusts a resumed transcript engine over the tab request', () => {
    const { m, internal } = manager({ engine: 'codex' }, '00000000-0000-4000-8000-000000000001')
    internal.dispatchMessage({ type: 'session:created', sessionId: 's1', engine: 'claude' })
    expect(m.getEngine()).toBe('claude')
  })

  it('asks for the GPT list and stores the main agent through the socket', () => {
    const { m, frames } = manager({})
    m.requestCodexModels()
    m.setDefaultEngine('codex')
    expect(frames).toEqual([{ type: 'codex:models' }, { type: 'agent:set-default-engine', engine: 'codex' }])
  })

  it('reports the main agent to the host callback', () => {
    const seen: string[] = []
    ConnectionManager.onAgentPreferences = (_tab, engine) => { seen.push(engine) }
    const { internal, sent } = manager({})
    internal.dispatchMessage({ type: 'agent:preferences', defaultEngine: 'codex' })
    internal.dispatchMessage({ type: 'agent:preferences', error: 'Unknown engine' })
    expect(seen).toEqual(['codex'])
    expect(sent.filter((e) => e.channel === 'agent-preferences').map((e) => e.args[1]))
      .toEqual([{ defaultEngine: 'codex' }, { error: 'Unknown engine' }])
  })
})

describe('engine replies relayed to the webview', () => {
  function ctx(): { ctx: HandlerCtx; sent: Array<{ channel: string; args: unknown[] }> } {
    const sent: Array<{ channel: string; args: unknown[] }> = []
    const value = {
      tabId: 'tab-1',
      window: { webContents: { send: (channel: string, ...args: unknown[]) => sent.push({ channel, args }) } },
    } as unknown as HandlerCtx
    return { ctx: value, sent }
  }

  it('forwards the GPT list with the account and normalizes a missing list', () => {
    const { ctx: c, sent } = ctx()
    const model = { id: 'gpt-5.5', model: 'gpt-5.5', displayName: 'GPT-5.5', description: '', isDefault: true, defaultReasoningEffort: null, reasoningEfforts: [] }
    handleServerMessage({ type: 'codex:models', models: [model], account: { loggedIn: false } }, c)
    handleServerMessage({ type: 'codex:models', error: { kind: 'tls', message: 'TLS failed' } } as never, c)
    expect(sent.map((e) => [e.channel, e.args[0], e.args[1]])).toEqual([
      ['codex-models', 'tab-1', { models: [model], account: { loggedIn: false } }],
      ['codex-models', 'tab-1', { models: [], error: { kind: 'tls', message: 'TLS failed' } }],
    ])
  })

  it('forwards the sign-in gate without the server session id', () => {
    const { ctx: c, sent } = ctx()
    handleServerMessage({ type: 'codex:auth', sessionId: 's1', loggedIn: false, email: null }, c)
    expect(sent).toEqual([{ channel: 'codex-auth', args: ['tab-1', { loggedIn: false, email: null }] }])
  })

  it('forwards the engines sign-in state with the main agent', () => {
    const { ctx: c, sent } = ctx()
    const notified: string[] = []
    c.notifyAgentPreferences = (engine) => notified.push(engine)
    const auth = { claude: { loggedIn: false, reason: 'Claude is not signed in on this Bridge server.' }, codex: { loggedIn: true, reason: null } }
    handleServerMessage({ type: 'agent:preferences', defaultEngine: 'claude', auth }, c)
    expect(sent).toEqual([{ channel: 'agent-preferences', args: ['tab-1', { defaultEngine: 'claude', auth }] }])
    expect(notified).toEqual(['claude'])
  })

  it('tells the webview why a new chat runs on the other engine', () => {
    const { ctx: c, sent } = ctx()
    const noop = () => undefined
    Object.assign(c, {
      resetReconnectAttempt: noop, noteSessionCreated: noop, noteIdentityConfirmed: noop, setState: noop,
      setSettingsDir: noop, notifySessionInfo: noop, notifyAuthenticated: noop,
    })
    const engineNotice = 'Claude is not signed in on this Bridge server. This chat runs ChatGPT instead; sign Claude in from the Bridge dashboard.'
    handleServerMessage({ type: 'session:created', sessionId: 's1', engine: 'codex', engineNotice }, c)
    expect(sent.find((e) => e.channel === 'session-restarted')?.args).toEqual([
      'tab-1',
      { sessionId: 's1', effort: undefined, model: undefined, engine: 'codex', engineNotice },
    ])
  })
})
