// ============================================================================
// One codex app-server per CODEX_HOME (one per Bridge server), one thread per
// Bridge session.
//
// A single process per home means a single writer of auth.json: codex
// refreshes the ChatGPT token in-process, so two processes sharing one home
// could race on the refresh token. The registry below guarantees one live
// host per home even under concurrent session starts.
//
// Notifications are routed by `params.threadId`. A thread's first
// notifications can be dispatched before the caller has processed the
// thread/start response (both arrive on one ordered stream), so frames for a
// not-yet-attached thread are buffered (bounded) and replayed on attach.
// ============================================================================

import type { CodexExit } from './app-server'
import { CodexAppServer, type CodexAppServerOptions } from './app-server'
import { JsonRpcError, METHOD_NOT_FOUND, type JsonRpcConnection } from './jsonrpc'

/** What the host needs from a process; CodexAppServer in production, a fake in tests. */
export interface CodexProcess {
  readonly rpc: JsonRpcConnection
  readonly exited: CodexExit | undefined
  onExit(listener: (exit: CodexExit) => void): () => void
  close(graceMs?: number): Promise<CodexExit>
}

export interface ThreadSink {
  onNotification(method: string, params: unknown, emittedAtMs: unknown): void
  onExit(exit: CodexExit): void
}

type Frame = { method: string; params: unknown; emittedAtMs: unknown }

const DECLINED = 'Bridge does not let codex run this itself; Bridge tools are the only way to act.'

/**
 * Answers for codex→Bridge requests. Built-in exec/patch are already removed
 * by config; declining their approvals is the second line. Elicitations and
 * request_user_input are declined until the Bridge UI owns them, and Bridge
 * registers no dynamic tools (every tool is Bridge MCP, which survives resume).
 */
export function answerServerRequest(method: string): unknown {
  switch (method) {
    case 'item/commandExecution/requestApproval':
    case 'item/fileChange/requestApproval':
      return { decision: 'decline' }
    case 'execCommandApproval':
    case 'applyPatchApproval':
      return { decision: { denied: { rejection: DECLINED } } }
    case 'mcpServer/elicitation/request':
      return { action: 'decline', content: null, _meta: null }
    case 'item/tool/call':
      return {
        success: false,
        contentItems: [{ type: 'inputText', text: 'No dynamic tools are registered by Bridge.' }],
      }
    default:
      throw new JsonRpcError(METHOD_NOT_FOUND, `${method} is not supported by Bridge`)
  }
}

export interface CodexHostOptions {
  /** Close the process this long after its last thread detached. */
  idleCloseMs?: number
  maxBufferedPerThread?: number
  maxBufferedThreads?: number
}

export class CodexHost {
  private readonly sinks = new Map<string, ThreadSink>()
  private readonly early = new Map<string, Frame[]>()
  private readonly retired = new Set<string>()
  private readonly hostListeners = new Set<(method: string, params: unknown) => void>()
  private idleTimer: NodeJS.Timeout | undefined
  private closed = false
  private exitedUnexpectedly = false
  private readonly closeListeners = new Set<() => void>()

  constructor(
    readonly process: CodexProcess,
    private readonly options: CodexHostOptions = {},
  ) {
    process.rpc.setServerRequestHandler((method) => answerServerRequest(method))
    process.rpc.onNotification((method, params, frame) => this.route(method, params, frame.emittedAtMs))
    process.onExit((exit) => {
      // Exit without close(): the app-server died under its threads.
      if (!this.closed) this.exitedUnexpectedly = true
      this.closed = true
      clearTimeout(this.idleTimer)
      for (const sink of [...this.sinks.values()]) sink.onExit(exit)
      this.sinks.clear()
      this.early.clear()
      for (const listener of this.closeListeners) listener()
    })
  }

  static async start(options: CodexAppServerOptions, hostOptions?: CodexHostOptions): Promise<CodexHost> {
    return new CodexHost(await CodexAppServer.start(options), hostOptions)
  }

  get rpc(): JsonRpcConnection {
    return this.process.rpc
  }

  get alive(): boolean {
    return !this.closed && !this.process.exited
  }

  get threadCount(): number {
    return this.sinks.size
  }

  /** The process exited without being closed (crash, kill) — not an idle close. */
  get crashed(): boolean {
    return this.exitedUnexpectedly
  }

  /** A Bridge session already owns this thread on this app-server. */
  isAttached(threadId: string): boolean {
    return this.sinks.has(threadId)
  }

  /** Account-level notifications (rate limits, login, account) without a thread. */
  onHostNotification(listener: (method: string, params: unknown) => void): () => void {
    this.hostListeners.add(listener)
    return () => this.hostListeners.delete(listener)
  }

  onClose(listener: () => void): () => void {
    this.closeListeners.add(listener)
    return () => this.closeListeners.delete(listener)
  }

  private route(method: string, params: unknown, emittedAtMs: unknown): void {
    const threadId = (params as { threadId?: unknown } | undefined)?.threadId
    const id =
      typeof threadId === 'string' ? threadId : method === 'thread/started' ? (params as any)?.thread?.id : undefined
    if (typeof id !== 'string') {
      for (const listener of this.hostListeners) listener(method, params)
      return
    }
    const sink = this.sinks.get(id)
    if (sink) {
      sink.onNotification(method, params, emittedAtMs)
      return
    }
    if (this.retired.has(id)) return
    let frames = this.early.get(id)
    if (!frames) {
      const maxThreads = this.options.maxBufferedThreads ?? 32
      if (this.early.size >= maxThreads) this.early.delete(this.early.keys().next().value!)
      frames = []
      this.early.set(id, frames)
    }
    if (frames.length < (this.options.maxBufferedPerThread ?? 1_000)) frames.push({ method, params, emittedAtMs })
  }

  /** Routes the thread's notifications to `sink`, replaying any that arrived first. */
  attach(threadId: string, sink: ThreadSink): () => void {
    if (!this.alive) throw new Error('codex host is closed')
    // One writer per thread: a second session on a live thread would interleave
    // turns on the same rollout (field lesson: a duplicate broke the thread).
    const owner = this.sinks.get(threadId)
    if (owner && owner !== sink) throw new CodexThreadBusyError(threadId)
    clearTimeout(this.idleTimer)
    this.idleTimer = undefined
    this.retired.delete(threadId)
    this.sinks.set(threadId, sink)
    const frames = this.early.get(threadId) ?? []
    this.early.delete(threadId)
    for (const f of frames) sink.onNotification(f.method, f.params, f.emittedAtMs)
    return () => this.detach(threadId, sink)
  }

  private detach(threadId: string, sink: ThreadSink): void {
    if (this.sinks.get(threadId) !== sink) return
    this.sinks.delete(threadId)
    this.retired.add(threadId)
    if (this.retired.size > 1_000) this.retired.delete(this.retired.values().next().value!)
    if (this.sinks.size === 0 && this.alive) {
      this.idleTimer = setTimeout(() => void this.close(), this.options.idleCloseMs ?? 5 * 60_000)
      this.idleTimer.unref?.()
    }
  }

  async close(graceMs?: number): Promise<void> {
    clearTimeout(this.idleTimer)
    this.closed = true
    await this.process.close(graceMs)
  }
}

export class CodexThreadBusyError extends Error {
  constructor(readonly threadId: string) {
    super('this ChatGPT conversation is already open in another Bridge session')
    this.name = 'CodexThreadBusyError'
  }
}

export class CodexRestartPausedError extends Error {
  constructor(
    readonly crashes: number,
    readonly until: number,
  ) {
    super(
      `the codex app-server crashed ${crashes} times in a row; restarts are paused until ${new Date(until).toISOString().slice(11, 16)} UTC`,
    )
    this.name = 'CodexRestartPausedError'
  }
}

/** When a crashed app-server may be started again. Defaults follow the agent
 *  driver's field data: a crash within 2 min of its start twice in a row is a
 *  crash storm (84 empty restarts in one night), and so is any 5 crashes in
 *  10 min; both pause restarts for 5 min instead of looping. */
export interface RestartPolicy {
  now?: () => number
  stormMs?: number
  stormCrashes?: number
  windowMs?: number
  maxCrashes?: number
  cooldownMs?: number
}

/** One host per CODEX_HOME; concurrent acquirers share the same start. Dead
 *  hosts are restarted on the next acquire, within the restart policy. */
export class CodexHostRegistry {
  private readonly suspended = new Set<string>()
  private readonly hosts = new Map<string, Promise<CodexHost>>()
  private readonly history = new Map<
    string,
    { startedAt: number; crashes: number[]; quick: number; pausedUntil: number }
  >()

  constructor(
    private readonly startHost: (home: string) => Promise<CodexHost>,
    private readonly policy: RestartPolicy = {},
  ) {}

  acquire(home: string): Promise<CodexHost> {
    if (this.suspended.has(home))
      return Promise.reject(
        new Error('Codex terminal is using this account; exit it before starting a chat or changing the account'),
      )
    const existing = this.hosts.get(home)
    if (existing) {
      return existing.then((host) => (host.alive ? host : this.restart(home, existing)))
    }
    const paused = this.pausedUntil(home)
    if (paused) return Promise.reject(new CodexRestartPausedError(this.history.get(home)!.crashes.length, paused))
    return this.launch(home)
  }

  /** Hand the home to a CLI only after the app-server has fully exited. */
  async suspend(home: string): Promise<() => void> {
    if (this.suspended.has(home)) throw new Error('A Codex terminal is already running')
    this.suspended.add(home)
    try {
      const host = await this.hosts.get(home)
      if (host?.alive && host.threadCount > 0)
        throw new Error('Exit active Codex chats before opening the Codex terminal')
      await host?.close()
      return () => {
        this.suspended.delete(home)
      }
    } catch (error) {
      this.suspended.delete(home)
      throw error
    }
  }

  private pausedUntil(home: string): number | null {
    const h = this.history.get(home)
    const now = (this.policy.now ?? Date.now)()
    return h && h.pausedUntil > now ? h.pausedUntil : null
  }

  private noteCrash(home: string): void {
    const now = (this.policy.now ?? Date.now)()
    const h = this.history.get(home)
    if (!h) return
    const windowMs = this.policy.windowMs ?? 10 * 60_000
    h.crashes = [...h.crashes.filter((at) => now - at < windowMs), now]
    h.quick = now - h.startedAt < (this.policy.stormMs ?? 2 * 60_000) ? h.quick + 1 : 0
    if (h.quick >= (this.policy.stormCrashes ?? 2) || h.crashes.length >= (this.policy.maxCrashes ?? 5)) {
      h.pausedUntil = now + (this.policy.cooldownMs ?? 5 * 60_000)
    }
  }

  private restart(home: string, stale: Promise<CodexHost>): Promise<CodexHost> {
    if (this.hosts.get(home) === stale) this.hosts.delete(home)
    return this.acquire(home)
  }

  private launch(home: string): Promise<CodexHost> {
    const now = (this.policy.now ?? Date.now)()
    const prior = this.history.get(home)
    this.history.set(home, {
      startedAt: now,
      crashes: prior?.crashes ?? [],
      quick: prior?.quick ?? 0,
      pausedUntil: 0,
    })
    const started = this.startHost(home).then((host) => {
      host.onClose(() => {
        if (host.crashed) this.noteCrash(home)
        if (this.hosts.get(home) === started) this.hosts.delete(home)
      })
      return host
    })
    this.hosts.set(home, started)
    started.catch(() => {
      if (this.hosts.get(home) === started) this.hosts.delete(home)
    })
    return started
  }

  async closeAll(): Promise<void> {
    const all = [...this.hosts.values()]
    this.hosts.clear()
    await Promise.all(all.map((p) => p.then((h) => h.close()).catch(() => undefined)))
  }
}
