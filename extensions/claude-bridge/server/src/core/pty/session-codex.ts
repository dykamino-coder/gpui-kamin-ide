// ============================================================================
// A Bridge session whose agent is a codex thread instead of a Claude CLI PTY.
//
// The session is an ordinary PtySession record, so everything keyed on it
// keeps working unchanged: the per-session MCP endpoint and its Bearer (the
// real mcp-http-handler and its permission pipeline), the client WS binding,
// detach/reattach, admission, teardown and the session tree. What differs:
//   - `pty` is a VirtualPty: kill() stops the thread, Ctrl+C interrupts it;
//   - `agentInput` takes submitted text (session-io delegates to it);
//   - the transcript is Bridge's normalized projection (engine: "codex"),
//     appended to <projects>/<slug(settingsDir)>/<threadId>.jsonl, where the
//     shared JsonlWatcher tails, replays and pages it like a CLI transcript.
// codex's own rollout (CODEX_HOME) stays the source of truth for resume.
// ============================================================================

import { randomUUID } from 'crypto'
import fs from 'fs'
import os from 'os'
import path from 'path'
import type { WebSocket as WS } from 'ws'
import type { AgentEvent } from '../../shared/agent-events'
import { config as bridgeConfig } from '../config'
import { CodexAgentSession } from '../codex/codex-session'
import { classifyCodexError, describeCodexError, loginRequired } from '../codex/codex-errors'
import { codexUnavailableReason, type CodexRuntime } from '../codex/codex-runtime'
import { bridgeMcpThreadConfig } from '../codex/config-toml'
import { JsonRpcError } from '../codex/jsonrpc'
import { CodexConsoleRenderer, consoleBanner, consoleNotice } from '../codex/console-render'
import { CodexTranscriptProjector, type CodexTranscriptEntry } from '../codex/transcript'
import { VirtualPty } from '../codex/virtual-pty'
import { eventBus } from '../events/bus'
import { debugLog, warnLog } from '../logging'
import { lifecycleLog } from '../logging/lifecycle'
import { bridgeClaudeMd } from './bridge-default-claude-md'
import { computeProjectSlug } from './jsonl-watcher'
import { holdMcpEndpoint, mcpIdentity } from './mcp-tasks'
import { ResumeNotFoundError } from './session-resume-not-found'
import { attachOutputDebounce, sendToClient } from './session-io'
import { attachSessionJsonlWatcher } from './session-jsonl-wiring'
import { buildSystemPrompt } from './system-prompt'
import type { PtySession, SessionConfig } from './types'

export interface CodexSessionDeps {
  runtime: CodexRuntime
  /** Registers the session (sessions map + admission release). */
  register(session: PtySession): void
  /** Same finalizer the PTY exit handler of a Claude session calls. */
  finalizeExit(sessionId: string, exitCode: number): void
  recordCompactLink(oldId: string, newId: string): void
  /** Where Claude-compatible transcripts live (tests point it at a temp dir). */
  projectsDir?: string
  /** A running turn with no codex event (and no tool running) for this long is
   *  reported as stalled. Default BRIDGE_CODEX_STALL_MS or 5 min. */
  stallMs?: number
}

const DEFAULT_STALL_MS = 5 * 60_000
const DEAD_NOTICE_GRACE_MS = 1_500

export interface CodexBindingHooks {
  /** Resume the thread on the runtime's current app-server. */
  reopen(threadId: string, model: string | null): Promise<CodexAgentSession>
  /** The thread cannot continue: end the Bridge session with this code. */
  onDead(code: number): void
  /** Bridge Console text (the virtual PTY's output). */
  console?(data: string): void
  stallMs?: number
}

export interface CodexPlacement {
  settingsDir: string
  /** Thread to resume (the conversation id the client holds). */
  resumeThreadId?: string
  resumeNotFound: boolean
}

const REASONING_EFFORTS = new Set(['none', 'minimal', 'low', 'medium', 'high', 'xhigh'])
const STREAM_THROTTLE_MS = 100

export function projectsDir(deps?: { projectsDir?: string }): string {
  return deps?.projectsDir ?? path.join(os.homedir(), '.claude', 'projects')
}

export function codexTranscriptPath(settingsDir: string, threadId: string, dir = projectsDir()): string {
  return path.join(dir, computeProjectSlug(settingsDir), `${threadId}.jsonl`)
}

export { transcriptEngine } from './transcript-engine'

function lastUuid(file: string): string | null {
  try {
    const lines = fs.readFileSync(file, 'utf8').trimEnd().split('\n')
    for (let i = lines.length - 1; i >= 0; i--) {
      try {
        const uuid = (JSON.parse(lines[i]!) as { uuid?: unknown }).uuid
        if (typeof uuid === 'string') return uuid
      } catch {
        /* torn line */
      }
    }
  } catch {
    /* no file yet */
  }
  return null
}

/** The model and effort chosen in a codex chat. codex records them in its
 *  rollout only with the next turn, so a choice made without a turn would be
 *  lost when the app-server or Bridge restarts; Bridge keeps it next to the
 *  session settings, per thread, and resumes the thread with it. */
export interface CodexChoice {
  model?: string
  /** Bridge effort level (the selector's value). */
  effort?: string
}

function choiceFile(settingsDir: string, threadId: string): string {
  return path.join(settingsDir, `codex-choice-${threadId.replace(/[^\w.-]/g, '_')}.json`)
}

export function loadCodexChoice(settingsDir: string, threadId: string): CodexChoice | null {
  try {
    const raw = JSON.parse(fs.readFileSync(choiceFile(settingsDir, threadId), 'utf8')) as Record<string, unknown>
    return {
      ...(typeof raw.model === 'string' && raw.model ? { model: raw.model } : {}),
      ...(typeof raw.effort === 'string' && raw.effort ? { effort: raw.effort } : {}),
    }
  } catch {
    return null
  }
}

/** Records a model or effort change of a codex chat (merged with earlier ones). */
export function saveCodexChoice(session: PtySession, change: CodexChoice): void {
  const threadId = codexBindings.get(session)?.agent.conversationId()
  if (!threadId) return
  const next = { ...loadCodexChoice(session.settingsDir, threadId), ...change }
  try {
    fs.mkdirSync(session.settingsDir, { recursive: true })
    fs.writeFileSync(choiceFile(session.settingsDir, threadId), JSON.stringify(next), { mode: 0o600 })
  } catch (error) {
    warnLog('codex choice not saved', { sessionId: session.id, error: String(error) })
  }
}

/** Claude model ids from the shared selector never reach codex. */
export function codexModel(model: string | null | undefined): string | undefined {
  return model && !/^claude/i.test(model) ? model : undefined
}

/** Bridge effort levels → codex ReasoningEffort; Claude's `max` is codex `xhigh`. */
export function codexEffort(effort: string | null | undefined): string | undefined {
  if (effort === 'max') return 'xhigh'
  return effort && REASONING_EFFORTS.has(effort) ? effort : undefined
}

function developerInstructions(config: SessionConfig): string {
  // Same session context the Claude CLI receives, with Bridge tool names as
  // codex sees them (server `bridge` instead of Claude's `user-tools`).
  const text = [
    buildSystemPrompt(config.cwd, config.basePrompt, config.transcriptMirrorDir, config.host),
    bridgeClaudeMd(config.host),
  ].join('\n\n')
  return text.replaceAll('mcp__user-tools__', 'mcp__bridge__')
}

export async function createCodexSession(
  ws: WS,
  userName: string,
  tokenId: string,
  config: SessionConfig,
  placement: CodexPlacement,
  deps: CodexSessionDeps,
  signal?: AbortSignal,
): Promise<PtySession> {
  const sessionId = randomUUID()
  const mcpToken = randomUUID()
  const { settingsDir } = placement
  let resumeNotFound = placement.resumeNotFound
  fs.mkdirSync(settingsDir, { recursive: true })
  const mcpEndpoint = await mcpIdentity(tokenId, settingsDir)
  signal?.throwIfAborted()

  // A server without a working codex refuses the chat with the reason instead
  // of a raw spawn error (the selector already shows the group as unavailable).
  const availability = await deps.runtime.availability()
  if (!availability.available) throw new Error(`ChatGPT is unavailable: ${availability.reason}`)
  const host = await deps.runtime.host().catch((error: unknown) => {
    throw new Error(`ChatGPT is unavailable: ${codexUnavailableReason(error)}`, { cause: error })
  })
  signal?.throwIfAborted()
  const threadConfig = bridgeMcpThreadConfig({
    url: `http://127.0.0.1:${bridgeConfig.port}/mcp/${mcpEndpoint.id}`,
    token: mcpEndpoint.token,
  })
  // A resumed thread continues on the model and effort last chosen in it.
  const saved = placement.resumeThreadId ? loadCodexChoice(settingsDir, placement.resumeThreadId) : null
  const effort = saved?.effort ?? config.effort
  const startOptions = {
    host,
    id: sessionId,
    cwd: settingsDir,
    model: codexModel(saved?.model) ?? codexModel(config.model),
    effort: codexEffort(effort),
    threadConfig,
    developerInstructions: developerInstructions(config),
  }
  // The binding is created below; the virtual PTY always drives its current
  // agent, which changes when the thread is resumed on a restarted app-server.
  let binding: CodexSessionBinding
  const vpty = new VirtualPty(
    {
      interrupt: () => void binding.agent.interrupt().catch(() => undefined),
      stop: async () => {
        await binding.stop('session destroyed')
        return 0
      },
    },
    config.cols ?? 120,
    config.rows ?? 40,
  )

  // Built before the thread: codex initializes the Bridge MCP server while it
  // creates the thread, so the endpoint must already answer (holdMcpEndpoint).
  const session: PtySession = {
    id: sessionId,
    engine: 'codex',
    pty: vpty.asPty(),
    ws,
    userName,
    tokenId,
    settingsDir,
    cwd: config.cwd || '',
    state: 'running',
    createdAt: new Date(),
    lastActivityAt: new Date(),
    mcpCallCount: 0,
    inputCount: 0,
    userMessages: 0,
    assistantMessages: 0,
    contextTokens: 0,
    totalTokens: 0,
    mcpLog: [],
    mcpInitialized: false,
    mcpLastError: null,
    mcpToken,
    mcpEndpoint,
    outputBuffer: [],
    outputBufferBytes: 0,
    lastResizeAt: Date.now(),
    consoleShrunk: false,
    host: config.host,
    registeredTools: [],
    registeredResources: [],
    registeredResourceTemplates: [],
    registeredPrompts: [],
    cliConversationId: null,
    isSubAgent: config.isSubAgent || false,
    parentSessionId: config.parentSessionId || null,
    childSessions: [],
    pendingElicitations: new Map(),
    sessionTitle: null,
    isRestarting: false,
    effort: effort || null,
    model: startOptions.model ?? null,
    bearerHash: config.bearerHash,
    streamProtocol: config.protocolVersion ?? 0,
    resumeNotFound,
  }
  const releaseEndpoint = holdMcpEndpoint(session)
  let agent: CodexAgentSession
  try {
    try {
      agent = await CodexAgentSession.start({ ...startOptions, resumeThreadId: placement.resumeThreadId })
    } catch (error) {
      // The transcript survived but codex no longer has the thread's rollout.
      if (!placement.resumeThreadId || !(error instanceof JsonRpcError)) {
        throw new Error(describeCodexError(classifyCodexError((error as Error).message)), { cause: error })
      }
      if (config.noSpawnOnNotFound) throw new ResumeNotFoundError()
      warnLog('codex resume: thread not found — starting a fresh thread', { threadId: placement.resumeThreadId })
      resumeNotFound = true
      agent = await CodexAgentSession.start(startOptions)
    }
    if (signal?.aborted) {
      await agent.stop('startup_aborted')
      signal.throwIfAborted()
    }
  } catch (error) {
    releaseEndpoint()
    throw error
  }
  const threadId = agent.conversationId()!
  const transcript = codexTranscriptPath(settingsDir, threadId, projectsDir(deps))
  const resumed = Boolean(placement.resumeThreadId) && !resumeNotFound
  session.model = agent.model
  session.cliConversationId = resumed ? threadId : null
  session.resumeNotFound = resumeNotFound
  binding = new CodexSessionBinding(session, agent, deps.runtime, transcript, config.cwd, {
    // Same thread, same MCP endpoint and Bearer, on whichever app-server the
    // runtime has now (a crashed one is restarted within its restart limits).
    reopen: async (threadId, model) =>
      CodexAgentSession.start({
        ...startOptions,
        host: await deps.runtime.host(),
        resumeThreadId: threadId,
        model: model ?? undefined,
        effort: codexEffort(session.effort),
      }),
    onDead: (code) => vpty.markExited(code),
    console: (data) => vpty.emit(data),
    stallMs: deps.stallMs,
  })
  session.agentInput = binding
  codexBindings.set(session, binding)

  deps.register(session)
  releaseEndpoint()
  // Bridge Console: the thread's live log through the virtual PTY, buffered
  // for reattach like CLI output. The banner is the console's first frame.
  attachOutputDebounce(session)
  vpty.emit(consoleBanner({ threadId, model: session.model, resumed, cwd: config.cwd || undefined }))

  // A resumed transcript exists already (epoch 0 finds it); a fresh one
  // appears with the first projected entry.
  attachSessionJsonlWatcher(session, config, resumed ? 0 : Date.now(), {
    recordCompactLink: deps.recordCompactLink,
    engine: 'codex',
  })

  vpty.onExit(({ exitCode }) => {
    lifecycleLog('pty_exit', { sessionId, exitCode, suppressed: false })
    session.state = 'exited'
    binding.dispose()
    sendToClient(session.ws, { type: 'session:exit', code: exitCode, sessionId })
    deps.finalizeExit(sessionId, exitCode)
  })

  eventBus.emit('session:created', {
    sessionId,
    tokenId,
    userName,
    cwd: config.cwd || '',
    state: 'running' as const,
    createdAt: session.createdAt.toISOString(),
    lastActivityAt: session.lastActivityAt.toISOString(),
    mcpCallCount: 0,
    inputCount: 0,
  })
  lifecycleLog('session_create', {
    sessionId,
    isSubAgent: Boolean(session.isSubAgent),
    resumeRequested: Boolean(placement.resumeThreadId),
    resumeConversationId: placement.resumeThreadId,
    resumeNotFound,
    settingsDirFound: Boolean(placement.resumeThreadId) && !placement.resumeNotFound,
    settingsDirSource: placement.resumeThreadId ? 'found' : 'fresh',
  })
  debugLog('codex session created', { sessionId, threadId, resumed })
  return session
}

const codexBindings = new WeakMap<PtySession, CodexSessionBinding>()

/** The codex side of a session, for the WS verbs that are not plain input. */
export function codexBinding(session: PtySession): CodexSessionBinding | undefined {
  return codexBindings.get(session)
}

/** Event stream of one codex session → transcript file, live stream and status frames. */
export class CodexSessionBinding {
  private readonly projector: CodexTranscriptProjector
  private offEvents: () => void
  private current: CodexAgentSession
  private stream: { id: string; text: string; sentAt: number; timer?: NodeJS.Timeout } | undefined
  private disposed = false
  private stopping = false
  /** Set while the thread is being resumed on a restarted app-server;
   *  submissions wait for it instead of failing. */
  private recovering: Promise<void> | undefined
  // Stall detector: a running turn that emits nothing while no tool runs.
  private turnOpen = false
  private readonly openTools = new Set<string>()
  private stallTimer: NodeJS.Timeout | undefined
  private stalled = false
  private readonly stallMs: number
  private readonly console = new CodexConsoleRenderer()

  constructor(
    private readonly session: PtySession,
    agent: CodexAgentSession,
    private readonly runtime: CodexRuntime,
    readonly transcriptPath: string,
    cwd?: string,
    private readonly hooks?: CodexBindingHooks,
  ) {
    this.current = agent
    this.stallMs = hooks?.stallMs ?? (Number(process.env.BRIDGE_CODEX_STALL_MS) || DEFAULT_STALL_MS)
    this.projector = new CodexTranscriptProjector(cwd || undefined, lastUuid(transcriptPath))
    this.offEvents = agent.events.subscribe((event) => this.onEvent(event))
  }

  /** A model change: the next turn's entries carry the new model. */
  modelChanged(model: string): void {
    this.projector.setModel(model)
  }

  /** The thread's live agent (replaced after an app-server restart). */
  get agent(): CodexAgentSession {
    return this.current
  }

  dispose(): void {
    this.disposed = true
    clearTimeout(this.stream?.timer)
    this.disarmStall()
    this.offEvents()
  }

  /** Ends the thread for good (session teardown). */
  async stop(reason: string): Promise<void> {
    this.stopping = true
    await this.recovering?.catch(() => undefined)
    await this.current.stop(reason)
  }

  // ── App-server restart ─────────────────────────────────────────────────
  // Field lesson: only a real process exit restarts anything — a completed
  // turn (or a final report) leaves the app-server and the thread as they are.

  private recover(code: number | null): void {
    if (this.recovering || this.stopping || this.disposed || !this.hooks) {
      if (!this.hooks && !this.stopping) this.markDead(code)
      return
    }
    const threadId = this.current.conversationId()
    if (!threadId) return this.markDead(code)
    this.turnOpen = false
    this.openTools.clear()
    this.disarmStall()
    this.flushStream()
    this.activity(false)
    this.appendSystem('The ChatGPT app-server stopped. Restarting it and resuming this conversation…', 'warning')
    lifecycleLog('codex_recover_start', { sessionId: this.session.id, threadId, code })
    this.recovering = (async () => {
      try {
        const next = await this.hooks!.reopen(threadId, this.current.model)
        if (this.stopping || this.disposed) {
          await next.stop('session destroyed')
          return
        }
        this.offEvents()
        this.current = next
        this.offEvents = next.events.subscribe((event) => this.onEvent(event))
        this.appendSystem(
          'Resumed the conversation on a restarted ChatGPT app-server. A reply that was in progress was cut off; send the message again if it is missing.',
          'warning',
        )
        lifecycleLog('codex_recover_done', { sessionId: this.session.id, threadId })
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error)
        this.appendSystem(`ChatGPT could not be restarted: ${reason}`, 'error')
        lifecycleLog('codex_recover_failed', { sessionId: this.session.id, threadId, reason })
        // Let the transcript watcher deliver the reason before the session ends.
        await new Promise((resolve) => setTimeout(resolve, DEAD_NOTICE_GRACE_MS))
        this.markDead(code)
      } finally {
        this.recovering = undefined
      }
    })()
  }

  private markDead(code: number | null): void {
    this.hooks?.onDead(code ?? 1)
  }

  // ── Stall detector ─────────────────────────────────────────────────────

  private armStall(): void {
    clearTimeout(this.stallTimer)
    this.stallTimer = undefined
    if (!this.turnOpen || this.openTools.size > 0 || this.disposed) return
    this.stallTimer = setTimeout(() => this.onStall(), this.stallMs)
    this.stallTimer.unref?.()
  }

  private disarmStall(): void {
    clearTimeout(this.stallTimer)
    this.stallTimer = undefined
    this.stalled = false
  }

  private onStall(): void {
    this.stallTimer = undefined
    if (!this.turnOpen || this.stalled) return
    this.stalled = true
    const minutes = Math.max(1, Math.round(this.stallMs / 60_000))
    this.appendSystem(
      `No response from ChatGPT for ${minutes} min; the reply is still running. Press Stop to interrupt it and send the message again.`,
      'warning',
    )
    lifecycleLog('codex_turn_stalled', { sessionId: this.session.id, stallMs: this.stallMs })
  }

  private trackStall(event: AgentEvent): void {
    switch (event.kind) {
      case 'turn.started':
        this.turnOpen = true
        this.openTools.clear()
        break
      case 'turn.completed':
      case 'session.exited':
        this.turnOpen = false
        this.openTools.clear()
        this.disarmStall()
        return
      case 'tool.started':
        this.openTools.add(event.id)
        break
      case 'tool.completed':
        this.openTools.delete(event.id)
        break
    }
    if (this.stalled) {
      this.stalled = false
      this.appendSystem('ChatGPT is responding again.', 'warning')
    }
    this.armStall()
  }

  // ── Input ──────────────────────────────────────────────────────────────

  submit(text: string, submissionId?: string): void {
    void this.deliver(text, submissionId)
  }

  input(data: string): boolean {
    if (!data.includes('\x03')) return false
    void this.agent.interrupt().catch(() => undefined)
    this.activity(false)
    return true
  }

  private async deliver(text: string, submissionId?: string): Promise<void> {
    // Queued behind an app-server restart, then sent to the resumed thread.
    if (this.recovering) await this.recovering
    const account = await this.runtime.accountStatus()
    if (!account.loggedIn) {
      // A failed account read (proxy 407, untrusted CA, no route) is shown as
      // such — never as "not signed in".
      const message =
        account.checkFailed && account.error
          ? describeCodexError(account.error)
          : (account.error && account.error.kind !== 'other' ? account.error : loginRequired()).message
      this.receipt(text, submissionId, 'uncertain', message)
      sendToClient(this.session.ws, { type: 'codex:auth', sessionId: this.session.id, ...account })
      this.appendSystem(message, 'error')
      return
    }
    try {
      await this.agent.send(text, submissionId)
      this.receipt(text, submissionId, 'read')
    } catch (error) {
      const info = classifyCodexError((error as Error).message)
      this.receipt(text, submissionId, 'uncertain', describeCodexError(info))
    }
  }

  private receipt(text: string, id: string | undefined, state: 'read' | 'uncertain', warning?: string): void {
    if (!id) return
    sendToClient(this.session.ws, {
      type: 'session:submission',
      sessionId: this.session.id,
      id,
      text,
      state,
      cancelable: false,
      ...(warning ? { warning } : {}),
      createdAt: Date.now(),
    })
  }

  // ── Output ─────────────────────────────────────────────────────────────

  private onEvent(raw: AgentEvent): void {
    if (this.disposed) return
    this.hooks?.console?.(this.console.render(this.explain(raw)))
    if (raw.kind === 'session.exited') {
      this.trackStall(raw)
      // Our own stop ends the thread; anything else is the app-server dying.
      if (raw.reason === 'codex app-server exited') this.recover(raw.code)
      return
    }
    this.trackStall(raw)
    const event = this.explain(raw)
    switch (event.kind) {
      case 'turn.started':
        this.activity(true)
        break
      case 'turn.completed':
        this.flushStream()
        this.activity(false)
        break
      case 'message.assistant.delta':
        this.streamDelta(event.id, event.delta)
        break
      case 'message.assistant':
        this.endStream(event.id)
        break
      case 'model.rerouted':
        this.session.model = event.to
        break
      case 'usage':
        this.session.contextTokens = event.last.inputTokens + event.last.cachedInputTokens
        this.session.totalTokens = event.total.totalTokens
        break
      case 'tool.started':
        this.session.mcpCallCount++
        break
    }
    const entries = this.projector.project(event)
    if (entries.length) this.append(entries)
  }

  /** Transport failures in plain language (TLS, 407, unauthorized). */
  private explain(event: AgentEvent): AgentEvent {
    if (event.kind === 'turn.completed' && event.status === 'failed' && event.error) {
      return {
        ...event,
        error: { ...event.error, message: describeCodexError(classifyCodexError(event.error.message)) },
      }
    }
    if (event.kind === 'notice' && event.level === 'error') {
      return { ...event, message: describeCodexError(classifyCodexError(event.message)) }
    }
    return event
  }

  private append(entries: CodexTranscriptEntry[]): void {
    try {
      fs.mkdirSync(path.dirname(this.transcriptPath), { recursive: true })
      fs.appendFileSync(this.transcriptPath, entries.map((e) => JSON.stringify(e) + '\n').join(''), { mode: 0o600 })
    } catch (error) {
      warnLog('codex transcript append failed', { sessionId: this.session.id, error: String(error) })
    }
  }

  private appendSystem(text: string, level: 'warning' | 'error'): void {
    this.hooks?.console?.(consoleNotice(level, text))
    const threadId = this.agent.conversationId()
    if (!threadId) return
    this.append(this.projector.project({ kind: 'notice', threadId, at: Date.now(), level, message: text }))
  }

  private activity(working: boolean): void {
    sendToClient(this.session.ws, {
      type: 'session:activity',
      sessionId: this.session.id,
      rawTitle: '',
      isWorking: working,
      hookDriven: true,
      promptReady: !working,
      waiting: false,
    })
  }

  // Live text: a streaming stub keyed by the item id; the projected entry
  // with the same message.id replaces it (webview mergeAssistantEntry).
  private streamDelta(id: string, delta: string): void {
    if (!this.stream || this.stream.id !== id) {
      this.flushStream()
      this.stream = { id, text: '', sentAt: 0 }
    }
    const s = this.stream
    const firstChunk = s.text === ''
    s.text += delta
    if ((this.session.streamProtocol ?? 0) >= 1 && !firstChunk) {
      sendToClient(this.session.ws, {
        type: 'streaming:delta',
        sessionId: this.session.id,
        msgId: id,
        blockIdx: 0,
        appendText: delta,
        kind: 'text',
      })
      return
    }
    if (firstChunk || Date.now() - s.sentAt >= STREAM_THROTTLE_MS) this.sendStub()
    else s.timer ??= setTimeout(() => this.sendStub(), STREAM_THROTTLE_MS)
  }

  private sendStub(): void {
    const s = this.stream
    if (!s) return
    clearTimeout(s.timer)
    s.timer = undefined
    s.sentAt = Date.now()
    sendToClient(this.session.ws, {
      type: 'streaming:entry',
      sessionId: this.session.id,
      entry: {
        type: 'assistant',
        uuid: `stream:${s.id}`,
        timestamp: new Date().toISOString(),
        engine: 'codex',
        message: { id: s.id, role: 'assistant', content: [{ type: 'text', text: s.text }] },
        __streaming: true,
      },
    })
  }

  private flushStream(): void {
    if (this.stream?.timer) this.sendStub()
    this.stream = undefined
  }

  private endStream(id: string): void {
    if (this.stream?.id === id) {
      clearTimeout(this.stream.timer)
      this.stream = undefined
    }
  }
}
