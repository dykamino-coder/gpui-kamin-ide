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
import { CodexTranscriptProjector, type CodexTranscriptEntry } from '../codex/transcript'
import { VirtualPty } from '../codex/virtual-pty'
import { eventBus } from '../events/bus'
import { debugLog, warnLog } from '../logging'
import { lifecycleLog } from '../logging/lifecycle'
import { bridgeClaudeMd } from './bridge-default-claude-md'
import { computeProjectSlug } from './jsonl-watcher'
import { mcpIdentity } from './mcp-tasks'
import { ResumeNotFoundError } from './session-resume-not-found'
import { sendToClient } from './session-io'
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
  const startOptions = {
    host,
    id: sessionId,
    cwd: settingsDir,
    model: codexModel(config.model),
    effort: codexEffort(config.effort),
    threadConfig,
    developerInstructions: developerInstructions(config),
  }
  let agent: CodexAgentSession
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
  const threadId = agent.conversationId()!
  const transcript = codexTranscriptPath(settingsDir, threadId, projectsDir(deps))
  const resumed = Boolean(placement.resumeThreadId) && !resumeNotFound

  const vpty = new VirtualPty(
    {
      interrupt: () => void agent.interrupt().catch(() => undefined),
      stop: async () => {
        await agent.stop('session destroyed')
        return 0
      },
    },
    config.cols ?? 120,
    config.rows ?? 40,
  )

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
    cliConversationId: resumed ? threadId : null,
    isSubAgent: config.isSubAgent || false,
    parentSessionId: config.parentSessionId || null,
    childSessions: [],
    pendingElicitations: new Map(),
    sessionTitle: null,
    isRestarting: false,
    effort: config.effort || null,
    model: agent.model,
    bearerHash: config.bearerHash,
    streamProtocol: config.protocolVersion ?? 0,
    resumeNotFound,
  }
  const binding = new CodexSessionBinding(session, agent, deps.runtime, transcript, config.cwd)
  session.agentInput = binding
  codexBindings.set(session, binding)

  deps.register(session)

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
  agent.events.subscribe((event) => {
    if (event.kind === 'session.exited' && event.reason !== 'session destroyed') {
      // The shared app-server died under the thread.
      vpty.markExited(event.code ?? 1)
    }
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
  private readonly offEvents: () => void
  private stream: { id: string; text: string; sentAt: number; timer?: NodeJS.Timeout } | undefined
  private disposed = false

  constructor(
    private readonly session: PtySession,
    readonly agent: CodexAgentSession,
    private readonly runtime: CodexRuntime,
    readonly transcriptPath: string,
    cwd?: string,
  ) {
    this.projector = new CodexTranscriptProjector(cwd || undefined, lastUuid(transcriptPath))
    this.offEvents = agent.events.subscribe((event) => this.onEvent(event))
  }

  dispose(): void {
    this.disposed = true
    clearTimeout(this.stream?.timer)
    this.offEvents()
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
    const account = await this.runtime.accountStatus()
    if (!account.loggedIn) {
      const info = account.error && account.error.kind !== 'other' ? account.error : loginRequired()
      this.receipt(text, submissionId, 'uncertain', info.message)
      sendToClient(this.session.ws, { type: 'codex:auth', sessionId: this.session.id, ...account })
      this.appendSystem(info.message, 'error')
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
