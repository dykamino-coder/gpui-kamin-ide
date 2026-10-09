import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { EventEmitter } from 'node:events'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
const h = vi.hoisted(() => ({
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
vi.mock('../server/ws', () => ({ registerWsRoute: vi.fn() }))
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
  writeInput: vi.fn(),
  submitText: vi.fn(),
  resizeTerminal: vi.fn(),
  handleMcpResponse: vi.fn(),
  handleMcpDenied: vi.fn(),
  handleElicitationResponse: vi.fn(),
  getSession: (id: string) => h.sessions.get(id),
  countUserSessions: () => 0,
  restartWithEffort: h.restartEffort,
  restartWithModel: h.restartModel,
  getSessionTree: () => [],
  deleteSessionByConversationId: vi.fn(),
}))
// Inert boundaries for the opt-in real PTY test. No user settings/provider.
vi.mock('./session-settings', () => ({
  get SESSIONS_BASE() {
    return h.root
  },
  sanitizeDirName: () => 'fixture',
  writeSessionSettings: vi.fn(),
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
import { attachSessionWebSocket, getSessionWs } from './session-ws'
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
const makeSession = (id: string, ws: Socket) => ({
  id,
  ws,
  tokenId: 'token',
  effort: 'high',
  model: 'test',
  settingsDir: 'fixture',
  registeredTools: [],
  jsonlWatcher: { replayAll: vi.fn() },
})
let socket: Socket
let sockets: Socket[] = []
beforeEach(() => {
  vi.useFakeTimers()
  h.create.mockReset()
  h.resolve.mockReset().mockResolvedValue({ userName: 'tester', tokenId: 'token' })
  h.detach.mockReset()
  h.destroy.mockReset()
  h.find.mockReset()
  h.sessions.clear()
  attachSessionWebSocket({} as any)
  sockets = []
  socket = new Socket()
  sockets.push(socket)
  h.wss.emit('connection', socket)
})
afterEach(() => {
  for (const client of sockets) client.close()
  vi.clearAllTimers()
  vi.useRealTimers()
})
describe('INC-2026-0013 actual WebSocket startup ownership', () => {
  it.each(['session:create', 'session:resume'])(
    'bounds late %s completion after close without stale indexes',
    async (type) => {
      let release!: (value: any) => void
      h.create.mockImplementation(
        () =>
          new Promise((resolve) => {
            release = resolve
          }),
      )
      const request = socket.message({ type, token: 'synthetic', conversationId: 'conversation' })
      await Promise.resolve()
      const session = makeSession(`late-${type}`, socket)
      h.sessions.set(session.id, session)
      socket.close()
      release(session)
      await request
      expect(h.detach).toHaveBeenCalledWith(session.id)
      expect(getSessionWs(session.id)).toBeUndefined()
      expect(socket.send.mock.calls.some(([raw]) => JSON.parse(raw).type === 'session:created')).toBe(false)
    },
  )
  it.each(['session:create', 'session:resume'])('ignores %s authentication completed after close', async (type) => {
    let release!: (value: any) => void
    h.resolve.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve
        }),
    )
    const request = socket.message({ type, token: 'synthetic', conversationId: 'conversation' })
    socket.close()
    release({ userName: 'tester', tokenId: 'token' })
    await request
    expect(h.create).not.toHaveBeenCalled()
    expect(socket.send).not.toHaveBeenCalled()
  })
  it('destroys late startup after explicit end instead of granting network grace', async () => {
    let release!: (value: any) => void
    h.create.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve
        }),
    )
    const request = socket.message({ type: 'session:create', token: 'synthetic' })
    await Promise.resolve()
    await socket.message({ type: 'session:end' })
    const session = makeSession('ended', socket)
    release(session)
    await request
    expect(h.destroy).toHaveBeenCalledWith('ended')
    expect(h.detach).not.toHaveBeenCalled()
    expect(getSessionWs('ended')).toBeUndefined()
  })
  it('cannot overwrite a replacement binding with the older completion', async () => {
    let old!: (value: any) => void
    let newer!: (value: any) => void
    h.create
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            old = resolve
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            newer = resolve
          }),
      )
    const first = socket.message({ type: 'session:create', token: 'synthetic' })
    await Promise.resolve()
    const second = socket.message({ type: 'session:create', token: 'synthetic' })
    await Promise.resolve()
    const current = makeSession('new', socket)
    h.sessions.set('new', current)
    newer(current)
    await second
    old(makeSession('old', socket))
    await first
    expect(h.destroy).toHaveBeenCalledWith('old')
    expect(getSessionWs('old')).toBeUndefined()
    expect(getSessionWs('new')).toBe(socket)
  })
  it('releases a resume lock and ignores a closed waiter', async () => {
    let release!: (value: any) => void
    h.create.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve
        }),
    )
    const first = socket.message({ type: 'session:resume', token: 'synthetic', conversationId: 'same' })
    await Promise.resolve()
    const waiter = new Socket()
    sockets.push(waiter)
    h.wss.emit('connection', waiter)
    const second = waiter.message({ type: 'session:resume', token: 'synthetic', conversationId: 'same' })
    await Promise.resolve()
    waiter.close()
    const session = makeSession('lock-owner', socket)
    h.sessions.set(session.id, session)
    release(session)
    await first
    await second
    expect(h.create).toHaveBeenCalledTimes(1)
    expect(getSessionWs(session.id)).toBe(socket)
  })
  it('does not detach a live session stolen by the replacement socket', async () => {
    const session = makeSession('stolen', socket)
    h.sessions.set(session.id, session)
    h.create.mockResolvedValue(session)
    await socket.message({ type: 'session:create', token: 'synthetic' })
    h.find.mockReturnValue(session)
    const newer = new Socket()
    sockets.push(newer)
    h.wss.emit('connection', newer)
    await newer.message({ type: 'session:resume', token: 'synthetic', conversationId: 'same' })
    expect(getSessionWs(session.id)).toBe(newer)
    expect(h.detach).not.toHaveBeenCalled()
  })
  it('drops indexes after teardown and does not report a late startup failure to a closed socket', async () => {
    const session = makeSession('destroyed', socket)
    h.sessions.set(session.id, session)
    h.create.mockResolvedValue(session)
    await socket.message({ type: 'session:create', token: 'synthetic' })
    h.events.get('session:destroyed')!({ data: { sessionId: session.id, tokenId: 'token' } })
    expect(getSessionWs(session.id)).toBeUndefined()
    let reject!: (value: any) => void
    h.create.mockImplementation(
      () =>
        new Promise((_, fail) => {
          reject = fail
        }),
    )
    const request = socket.message({ type: 'session:create', token: 'synthetic' })
    await Promise.resolve()
    socket.close()
    socket.send.mockClear()
    reject(new Error('synthetic startup error'))
    await request
    expect(socket.send).not.toHaveBeenCalled()
  })
  it('real CLOSED ws send without callback emits no second cleanup error', async () => {
    vi.useRealTimers()
    const closed = new WebSocket(null as any, undefined, { autoPong: true } as any)
    ;(closed as any)._readyState = WebSocket.CLOSED
    const error = vi.fn()
    closed.on('error', error)
    expect(() => closed.send(JSON.stringify({ type: 'session:created', sessionId: 'synthetic' }))).not.toThrow()
    await new Promise((resolve) => setImmediate(resolve))
    expect(error).not.toHaveBeenCalled()
  })
  it.each(['session:change-effort', 'session:change-model'])('releases a late %s restart after close', async (type) => {
    const initial = makeSession(`initial-${type}`, socket) as any
    initial.cliConversationId = 'conversation'
    h.sessions.set(initial.id, initial)
    h.create.mockResolvedValue(initial)
    await socket.message({ type: 'session:create', token: 'synthetic' })
    let release!: (value: any) => void
    const restart = type === 'session:change-effort' ? h.restartEffort : h.restartModel
    restart.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve
        }),
    )
    const request = socket.message({ type, effort: 'high', model: 'test' })
    socket.close()
    const late = makeSession(`restart-${type}`, socket)
    release(late)
    await request
    expect(h.detach).toHaveBeenCalledWith(late.id)
    expect(getSessionWs(late.id)).toBeUndefined()
  })

  it.skipIf(process.platform !== 'linux' || process.env.BRIDGE_PTY_ACCEPTANCE !== '1')(
    'disposable Linux gate: delayed startup, timeout, reattach and PTY cleanup',
    async () => {
      vi.useRealTimers()
      vi.stubEnv('OCB_DETACH_GRACE_MS', '200')
      h.root = fs.mkdtempSync(path.join(os.tmpdir(), 'startup-owner-test-'))
      fs.writeFileSync(path.join(h.root, 'claude'), '#!/bin/sh\nexec /bin/sleep 60\n', { mode: 0o700 })
      const core = await import('./session-core')
      const nativeWs = await vi.importActual<typeof import('ws')>('ws')
      const server = new nativeWs.WebSocketServer({ host: '127.0.0.1', port: 0 })
      server.on('connection', (ws) => h.wss.listeners('connection')[0](ws))
      await new Promise<void>((resolve) => server.once('listening', resolve))
      const address = server.address() as { port: number }
      let release!: () => void
      let ready!: () => void
      let detached!: () => void
      const started = new Promise<void>((resolve) => {
        ready = resolve
      })
      const gate = new Promise<void>((resolve) => {
        release = resolve
      })
      const lost = new Promise<void>((resolve) => {
        detached = resolve
      })
      let session!: Awaited<ReturnType<typeof core.createSession>>
      h.create.mockImplementation(async (...args: Parameters<typeof core.createSession>) => {
        session = await core.createSession(...args)
        session.cliConversationId = 'conversation'
        h.sessions.set(session.id, session)
        ready()
        await gate
        return session
      })
      h.detach.mockImplementation((id: string) => {
        core.detachSession(id)
        detached()
      })
      h.destroy.mockImplementation(core.destroySession)
      h.reattach.mockImplementation(core.reattachSession)
      h.find.mockImplementation(core.findSessionByConversation)
      const connect = async () => {
        const client = new nativeWs.WebSocket(`ws://127.0.0.1:${address.port}`)
        await new Promise<void>((resolve) => client.once('open', resolve))
        return client
      }
      try {
        const first = await connect()
        first.send(JSON.stringify({ type: 'session:create', token: 'synthetic' }))
        await started
        expect(() => process.kill(session.pty.pid, 0)).not.toThrow()
        const closed = new Promise<void>((resolve) => first.once('close', () => resolve()))
        first.close()
        await closed
        release()
        await lost
        expect(session.detachGraceTimer).toBeTruthy()
        expect(getSessionWs(session.id)).toBeUndefined()
        const next = await connect()
        const resumed = new Promise<void>((resolve) =>
          next.on('message', (raw) => {
            if (JSON.parse(raw.toString()).type === 'session:created') resolve()
          }),
        )
        next.send(JSON.stringify({ type: 'session:resume', token: 'synthetic', conversationId: 'conversation' }))
        await resumed
        expect(getSessionWs(session.id)?.readyState).toBe(WebSocket.OPEN)
        expect(session.detachGraceTimer).toBeNull()
        expect(h.create).toHaveBeenCalledTimes(1)
        const exited = new Promise<void>((resolve) => session.pty.onExit(() => resolve()))
        next.close()
        await exited
        expect(core.sessions.size).toBe(0)
        expect(getSessionWs(session.id)).toBeUndefined()
      } finally {
        release?.()
        for (const client of server.clients) client.terminate()
        await new Promise<void>((resolve) => server.close(() => resolve()))
        await Promise.all(
          [...core.sessions.values()].map(
            (remaining) =>
              new Promise<void>((resolve) => {
                remaining.pty.onExit(() => resolve())
                remaining.pty.kill()
              }),
          ),
        )
        vi.unstubAllEnvs()
        if (
          path.dirname(path.resolve(h.root)) === path.resolve(os.tmpdir()) &&
          path.basename(h.root).startsWith('startup-owner-test-')
        )
          fs.rmSync(h.root, { recursive: true, force: true })
      }
    },
    15_000,
  )
})
