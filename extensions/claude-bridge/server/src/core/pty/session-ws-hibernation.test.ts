import { ResumeNotFoundError } from './session-resume-not-found'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { EventEmitter } from 'node:events'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
const h = vi.hoisted(() => ({
  deleteData: vi.fn(),
  submit: vi.fn(),
  acquire: vi.fn(async (_ticket: unknown) => () => {}),
  promote: vi.fn(),
  hibernate: vi.fn(),
  handles: [] as any[],
  write: vi.fn(),
  rememberEffort: vi.fn(),
  wss: null as any,
  root: '',
  reattach: vi.fn((session: any, ws: any) => (session.ws = ws)),
  restartEffort: vi.fn(),
  restartModel: vi.fn(),
  create: vi.fn(),
  resolve: vi.fn(),
  detach: vi.fn(),
  destroy: vi.fn(),
  find: vi.fn(),
  sessions: new Map<string, any>(),
  events: new Map<string, (data: any) => void>(),
}))
vi.mock('ws', async (importOriginal) => {
  const actual = await importOriginal<typeof import('ws')>()
  const { EventEmitter } = await import('node:events')
  return {
    ...actual,
    WebSocketServer: class extends EventEmitter {
      constructor() {
        super()
        h.wss = this
      }
    },
  }
})
vi.mock('./effort-selection', () => ({
  effortForResume: () => null,
  forgetEffortSelection: vi.fn(),
  rememberEffortSelection: h.rememberEffort,
}))
vi.mock('../server/ws', () => ({ registerWsRoute: vi.fn() }))
vi.mock('./session-hibernation', () => ({
  acquireWakeSlot: (_limit: unknown, ticket: unknown) => h.acquire(ticket),
  promoteWake: (ticket: { urgent: boolean }) => h.promote(ticket),
  registerHibernationHandle: (handle: any) => {
    h.handles.push(handle)
    return () => {}
  },
}))
vi.mock('../auth/tokens', () => ({ resolveToken: h.resolve }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn(), infoLog: vi.fn() }))
vi.mock('../events/bus', () => ({
  eventBus: {
    emit: (name: string, data: any) => h.events.get(name)?.({ data }),
    on: (name: string, fn: (data: any) => void) => {
      h.events.set(name, fn)
      return () => {}
    },
  },
}))
vi.mock('../hooks/bridge-emitter', () => ({ emitBridgeEvent: vi.fn() }))
vi.mock('./session-manager', () => ({
  createSession: h.create,
  destroySession: h.destroy,
  detachSession: h.detach,
  reattachSession: h.reattach,
  findSessionByConversation: h.find,
  followCompactLinks: (id: string) => id,
  writeInput: h.write,
  submitText: h.submit,
  resizeTerminal: vi.fn(),
  handleMcpResponse: vi.fn(),
  handleMcpDenied: vi.fn(),
  handleElicitationResponse: vi.fn(),
  getSession: (id: string) => h.sessions.get(id),
  countUserSessions: () => 0,
  restartWithEffort: h.restartEffort,
  restartWithModel: h.restartModel,
  getSessionTree: () => [],
  deleteSessionByConversationId: h.deleteData,
  hibernateSession: h.hibernate,
}))
// Inert boundaries for the opt-in real PTY test. No user settings/provider.
vi.mock('./mcp-tasks', () => ({
  mcpIdentity: async () => ({ id: 'synthetic-endpoint', token: 'synthetic-endpoint-token' }),
  startMcpTaskDelivery: vi.fn(),
  settleMcpTask: vi.fn(async () => false),
  mcpTasks: { hasWorkingOwner: () => false },
}))
vi.mock('./session-settings', () => ({
  get SESSIONS_BASE() {
    return h.root
  },
  sanitizeDirName: () => 'fixture',
  writeSessionSettings: vi.fn(() => '{}'),
  writeSessionClaudeMd: vi.fn(),
  applySyncData: vi.fn(),
}))
vi.mock('./session-env', () => ({
  buildSessionEnv: () => ({ PATH: h.root, HOME: h.root }),
  modelForResume: (value: string) => value,
}))
vi.mock('./session-plugin-args', () => ({ buildSessionClaudeArgs: () => [] }))
vi.mock('./session-resume-helpers', () => ({
  findOrRecreateSettingsDir: () => h.root,
  xbasename: (value: string) => path.basename(value),
  repairTranscriptForResume: vi.fn(),
  resolveNewestInChain: () => null,
  lastModelEntryForResume: () => null,
  lastEffortEntryForResume: () => null,
}))
vi.mock('./jsonl-watcher', () => ({
  SKIP_LINE_MARKER: '__skip__',
  JsonlWatcher: class {
    start() {}
    stop() {}
    replayAll() {}
  },
}))
vi.mock('./session-io', () => ({
  sendToClient: vi.fn(),
  writeInputToSession: vi.fn(),
  submitTextToSession: vi.fn(),
  resizeSessionTerminal: vi.fn(),
  attachStartupAutoResponder: vi.fn(),
  attachOutputDebounce: vi.fn(),
  clearInputThrottle: vi.fn(),
}))
vi.mock('../proxy/native-mitm', () => ({ startNativeMitm: vi.fn() }))
vi.mock('../proxy/streaming-settings', () => ({ getStreamingSettings: async () => ({ enabled: false }) }))
vi.mock('./transcript-archive', () => ({ archiveTranscriptForSession: vi.fn(), dropArchivedTranscript: vi.fn() }))
import { WebSocket } from 'ws'
import { attachSessionWebSocket, getSessionWs, sleepAction } from './session-ws'
import { sendToClient } from './session-io'
import { SessionCliControl } from './session-cli-control'
import { setSessionPromptReady } from './session-input-coordinator'

class Socket extends EventEmitter {
  readyState: number = WebSocket.OPEN
  send = vi.fn()
  ping = vi.fn()
  terminate = vi.fn()
  close() {
    this.readyState = WebSocket.CLOSED
    this.emit('close')
  }
  message(message: any) {
    return this.listeners('message')[0]!(JSON.stringify(message))
  }
}

const makeSession = (id: string, ws: Socket, extra: Record<string, unknown> = {}) => ({
  id,
  ws,
  tokenId: 'token',
  effort: 'max',
  model: 'opus-test',
  settingsDir: '/sessions/fixture',
  cwd: '/work',
  cliConversationId: 'conv-1',
  streamProtocol: 2,
  lastActivityAt: new Date(0),
  pty: { cols: 100, rows: 30 },
  registeredTools: [{ name: 'user_tool', description: '', inputSchema: {} }],
  registeredResources: [],
  registeredResourceTemplates: [],
  registeredPrompts: [],
  launchConfig: { cwd: '/work', basePrompt: 'standing instructions', transcriptMirrorDir: '/mirror' },
  jsonlWatcher: { replayAll: vi.fn() },
  ...extra,
})

const sent = (socket: Socket) => socket.send.mock.calls.map(([frame]) => JSON.parse(frame))

let socket: Socket
beforeEach(() => {
  vi.useFakeTimers()
  h.create.mockReset()
  h.submit.mockReset()
  h.acquire.mockReset().mockImplementation(async () => () => {})
  h.promote.mockReset()
  h.find.mockReset()
  h.destroy.mockReset()
  h.hibernate.mockReset().mockResolvedValue(undefined)
  h.resolve.mockReset().mockResolvedValue({ userName: 'tester', tokenId: 'token' })
  h.sessions.clear()
  h.handles.length = 0
  vi.mocked(sendToClient).mockClear()
  attachSessionWebSocket({} as any)
  socket = new Socket()
  h.wss.emit('connection', socket)
})
afterEach(() => {
  socket.close()
  vi.clearAllTimers()
  vi.useRealTimers()
})

async function asleepChat(extra: Record<string, unknown> = {}) {
  const session = makeSession('s-old', socket, extra)
  h.sessions.set(session.id, session)
  h.create.mockResolvedValueOnce(session)
  await socket.message({ type: 'session:create', token: 'synthetic' })
  const handle = h.handles.at(-1)
  expect(handle.awakeSessionId()).toBe('s-old')
  expect(handle.hibernate()).toBe(true)
  h.sessions.delete(session.id)
  return { session, handle }
}

describe('idle hibernation on the session socket', () => {
  it('stops the CLI without telling the client the session ended', async () => {
    const { handle } = await asleepChat()
    expect(h.hibernate).toHaveBeenCalledWith('s-old')
    expect(handle.awakeSessionId()).toBeNull()
    expect(getSessionWs('s-old')).toBeUndefined()
    expect(vi.mocked(sendToClient).mock.calls.some(([, m]) => (m as any).type === 'session:exit')).toBe(false)
    // No per-second command polling for a sleeping chat.
    socket.send.mockClear()
    vi.mocked(sendToClient).mockClear()
    await vi.advanceTimersByTimeAsync(3000)
    expect(vi.mocked(sendToClient).mock.calls.some(([, m]) => (m as any).type === 'commands:request')).toBe(false)
  })

  it('keeps resize and tool registrations for the wake without waking', async () => {
    await asleepChat()
    await socket.message({ type: 'session:resize', cols: 140, rows: 50 })
    const tools = [{ name: 'new_tool', description: '', inputSchema: {} }]
    await socket.message({ type: 'mcp:register-external-tools', tools })
    await socket.message({ type: 'session:interrupt' })
    expect(h.create).toHaveBeenCalledTimes(1)

    const fresh = makeSession('s-new', socket)
    h.create.mockImplementationOnce(async () => {
      h.sessions.set(fresh.id, fresh)
      return fresh
    })
    await socket.message({ type: 'session:submitText', data: 'continue', submissionId: 'sub-1' })
    const [, user, tokenId, config] = h.create.mock.calls[1]!
    expect([user, tokenId]).toEqual(['tester', 'token'])
    expect(config).toMatchObject({
      cwd: '/work',
      basePrompt: 'standing instructions',
      transcriptMirrorDir: '/mirror',
      cols: 140,
      rows: 50,
      effort: 'max',
      model: 'opus-test',
      protocolVersion: 2,
      resumeConversationId: 'conv-1',
      reuseSettingsDir: '/sessions/fixture',
    })
    expect(config.carryRegistrations.registeredTools).toEqual(tools)
    expect(sent(socket)).toContainEqual(
      expect.objectContaining({ type: 'session:restarted', sessionId: 's-new', effort: 'max', model: 'opus-test' }),
    )
    expect(h.submit).toHaveBeenCalledWith('s-new', 'continue', 'sub-1')
    expect(getSessionWs('s-new')).toBe(socket)
    expect(h.handles.at(-1).awakeSessionId()).toBe('s-new')
  })

  it('wakes once for messages that arrive while the CLI starts, in order', async () => {
    await asleepChat()
    let started!: () => void
    const fresh = makeSession('s-new', socket)
    h.create.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          started = () => {
            h.sessions.set(fresh.id, fresh)
            resolve(fresh)
          }
        }),
    )
    const first = socket.message({ type: 'session:submitText', data: 'one', submissionId: 'a' })
    await vi.advanceTimersByTimeAsync(0)
    const second = socket.message({ type: 'session:submitText', data: 'two', submissionId: 'b' })
    await vi.advanceTimersByTimeAsync(0)
    expect(h.submit).not.toHaveBeenCalled()
    started()
    await Promise.all([first, second])
    expect(h.create).toHaveBeenCalledTimes(2)
    expect(h.submit.mock.calls).toEqual([
      ['s-new', 'one', 'a'],
      ['s-new', 'two', 'b'],
    ])
  })

  it('confirms a message at once and reports asleep, waking and awake', async () => {
    await asleepChat()
    const frames = () => vi.mocked(sendToClient).mock.calls.map(([, m]) => m as any)
    expect(frames()).toContainEqual(
      expect.objectContaining({
        type: 'session:activity',
        sessionId: 's-old',
        hibernation: 'asleep',
        promptReady: true,
      }),
    )
    let started!: () => void
    const fresh = makeSession('s-new', socket)
    h.create.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          started = () => {
            h.sessions.set(fresh.id, fresh)
            resolve(fresh)
          }
        }),
    )
    const pending = socket.message({ type: 'session:submitText', data: 'hello', submissionId: 'q-1' })
    await vi.advanceTimersByTimeAsync(0)
    // Before the CLI exists: the receipt and the waking status.
    expect(frames()).toContainEqual(
      expect.objectContaining({ type: 'session:submission', id: 'q-1', text: 'hello', state: 'queued' }),
    )
    expect(frames()).toContainEqual(
      expect.objectContaining({ type: 'session:activity', hibernation: 'waking', isWorking: true }),
    )
    expect(h.acquire).toHaveBeenLastCalledWith({ urgent: true })
    started()
    await pending
    expect(frames()).toContainEqual(
      expect.objectContaining({ type: 'session:activity', sessionId: 's-new', hibernation: 'awake', isWorking: false }),
    )
    expect(h.submit).toHaveBeenCalledWith('s-new', 'hello', 'q-1')
  })

  it('warms a sleeping chat without a message and promotes it when one arrives', async () => {
    await asleepChat()
    let granted!: () => void
    h.acquire.mockImplementationOnce(() => new Promise((resolve) => (granted = () => resolve(() => {}))))
    const fresh = makeSession('s-new', socket)
    h.create.mockImplementationOnce(async () => {
      h.sessions.set(fresh.id, fresh)
      return fresh
    })
    await socket.message({ type: 'session:warm' })
    expect(h.acquire).toHaveBeenLastCalledWith({ urgent: false })
    // A second warm-up while one runs starts nothing new.
    await socket.message({ type: 'session:warm' })
    expect(h.acquire).toHaveBeenCalledTimes(1)
    const pending = socket.message({ type: 'session:submitText', data: 'now', submissionId: 'w-1' })
    await vi.advanceTimersByTimeAsync(0)
    expect(h.promote).toHaveBeenCalledWith({ urgent: false })
    granted()
    await pending
    expect(h.create).toHaveBeenCalledTimes(2)
    expect(h.submit).toHaveBeenCalledWith('s-new', 'now', 'w-1')
  })

  it('wakes for a transcript request behind user messages, and a later message does not promote it twice', async () => {
    await asleepChat()
    let granted!: () => void
    h.acquire.mockImplementationOnce(() => new Promise((resolve) => (granted = () => resolve(() => {}))))
    const fresh = makeSession('s-new', socket, { jsonlWatcher: { replayAll: vi.fn(), getFilePath: () => null } })
    h.create.mockImplementationOnce(async () => {
      h.sessions.set(fresh.id, fresh)
      return fresh
    })
    const sync = socket.message({ type: 'jsonl:sync-request', knownUuids: [] })
    await vi.advanceTimersByTimeAsync(0)
    expect(h.acquire).toHaveBeenLastCalledWith({ urgent: false })
    // Another non-user frame while it boots does not jump the queue.
    const other = socket.message({ type: 'jsonl:segment-request', start: 0, end: 1 })
    await vi.advanceTimersByTimeAsync(0)
    expect(h.promote).not.toHaveBeenCalled()
    const typed = socket.message({ type: 'session:submitText', data: 'go', submissionId: 'u-1' })
    await vi.advanceTimersByTimeAsync(0)
    expect(h.promote).toHaveBeenCalledTimes(1)
    granted()
    await Promise.all([sync, other, typed])
    expect(h.create).toHaveBeenCalledTimes(2)
    expect(h.submit).toHaveBeenCalledWith('s-new', 'go', 'u-1')
  })

  it('does not wake for focus reports of the client terminal', async () => {
    await asleepChat()
    await socket.message({ type: 'session:input', data: '\x1b[O' })
    await socket.message({ type: 'session:input', data: '\x1b[I' })
    expect(h.create).toHaveBeenCalledTimes(1)
    const fresh = makeSession('s-new', socket)
    h.create.mockImplementationOnce(async () => {
      h.sessions.set(fresh.id, fresh)
      return fresh
    })
    await socket.message({ type: 'session:input', data: 'y' })
    expect(h.create).toHaveBeenCalledTimes(2)
    expect(h.acquire).toHaveBeenLastCalledWith({ urgent: true })
  })

  it('ignores a warm-up for a chat that is awake', async () => {
    const session = makeSession('s-awake', socket)
    h.sessions.set(session.id, session)
    h.create.mockResolvedValueOnce(session)
    await socket.message({ type: 'session:create', token: 'synthetic' })
    await socket.message({ type: 'session:warm' })
    expect(h.create).toHaveBeenCalledTimes(1)
    expect(h.acquire).not.toHaveBeenCalled()
  })

  it('holds the waking message until the new CLI takes input', async () => {
    await asleepChat()
    const fresh = makeSession('s-new', socket, { state: 'running' })
    new SessionCliControl(fresh as any)
    h.create.mockResolvedValueOnce(fresh)
    const pending = socket.message({ type: 'session:submitText', data: 'x', submissionId: 'x' })
    await vi.advanceTimersByTimeAsync(500)
    expect(h.submit).not.toHaveBeenCalled()
    setSessionPromptReady(fresh as any, true)
    await vi.advanceTimersByTimeAsync(50)
    await pending
    expect(h.submit).toHaveBeenCalledWith('s-new', 'x', 'x')
  })
  it('waits for the stopped CLI to finish before resuming its transcript', async () => {
    let finished!: () => void
    h.hibernate.mockReset().mockReturnValue(new Promise<void>((resolve) => (finished = resolve)))
    await asleepChat()
    const fresh = makeSession('s-new', socket)
    h.create.mockResolvedValueOnce(fresh)
    const pending = socket.message({ type: 'session:submitText', data: 'x', submissionId: 'x' })
    await vi.advanceTimersByTimeAsync(0)
    expect(h.create).toHaveBeenCalledTimes(1)
    finished()
    await pending
    expect(h.create).toHaveBeenCalledTimes(2)
  })

  it('does not start a second CLI when the chat already runs on another socket', async () => {
    await asleepChat()
    h.find.mockReturnValue({ id: 'elsewhere' })
    await socket.message({ type: 'session:submitText', data: 'x', submissionId: 'x' })
    expect(h.create).toHaveBeenCalledTimes(1)
    expect(h.submit).not.toHaveBeenCalled()
    expect(sendToClient).toHaveBeenCalledWith(socket, { type: 'session:exit', code: -1, sessionId: 's-old' })
  })

  it('reports a failed wake as an exited session so the client resumes it', async () => {
    await asleepChat()
    h.create.mockRejectedValueOnce(new Error('spawn failed'))
    await socket.message({ type: 'session:submitText', data: 'x', submissionId: 'x' })
    expect(sendToClient).toHaveBeenCalledWith(socket, { type: 'session:exit', code: -1, sessionId: 's-old' })
    expect(h.submit).not.toHaveBeenCalled()
  })

  it('starts an empty chat fresh with the same cwd and model', async () => {
    await asleepChat({ cliConversationId: null })
    h.create.mockResolvedValueOnce(makeSession('s-new', socket))
    await socket.message({ type: 'session:input', data: 'h' })
    const config = h.create.mock.calls[1]![3]
    expect(config.resumeConversationId).toBeUndefined()
    expect(config.reuseSettingsDir).toBeUndefined()
    expect(config).toMatchObject({ cwd: '/work', model: 'opus-test' })
  })

  it('ends a sleeping chat without spawning or destroying anything', async () => {
    await asleepChat()
    await socket.message({ type: 'session:end' })
    expect(h.create).toHaveBeenCalledTimes(1)
    expect(h.destroy).not.toHaveBeenCalled()
    await socket.message({ type: 'session:submitText', data: 'x', submissionId: 'x' })
    expect(h.create).toHaveBeenCalledTimes(1)
  })

  it('refuses to hibernate a socket that is starting a session', async () => {
    let resolve!: (s: unknown) => void
    h.create.mockReturnValueOnce(new Promise((r) => (resolve = r)))
    const pending = socket.message({ type: 'session:create', token: 'synthetic' })
    await vi.advanceTimersByTimeAsync(0)
    const handle = h.handles.at(-1)
    expect(handle.awakeSessionId()).toBeNull()
    expect(handle.hibernate()).toBe(false)
    resolve(makeSession('s1', socket))
    await pending
  })
})

describe('command report polling', () => {
  it('asks an idle chat for its command report every 10 s, not every second', async () => {
    const session = makeSession('s1', socket)
    h.sessions.set(session.id, session)
    h.create.mockResolvedValueOnce(session)
    await socket.message({ type: 'session:create', token: 'synthetic' })
    vi.mocked(sendToClient).mockClear()
    await vi.advanceTimersByTimeAsync(25_000)
    const requests = vi.mocked(sendToClient).mock.calls.filter(([, m]) => (m as any).type === 'commands:request')
    expect(requests.length).toBeGreaterThanOrEqual(2)
    expect(requests.length).toBeLessThanOrEqual(3)
  })
})

describe('sleepAction', () => {
  it('wakes only for messages that need the CLI', () => {
    for (const type of [
      'session:input',
      'session:submitText',
      'session:change-model',
      'session:change-effort',
      'jsonl:sync-request',
    ])
      expect(sleepAction(type)).toBe('wake')
    for (const type of [
      'session:resize',
      'mcp:response',
      'elicitation:response',
      'hook:response',
      'mcp:register-external-tools',
    ])
      expect(sleepAction(type)).toBe('ignore')
    for (const type of ['commands:list', 'commands:report', 'codex:models']) expect(sleepAction(type)).toBe('pass')
    for (const type of ['session:end', 'session:create', 'session:resume']) expect(sleepAction(type)).toBe('release')
  })
})
