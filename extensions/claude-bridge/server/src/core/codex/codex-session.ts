// ============================================================================
// AgentSession over one codex thread in the server's shared app-server.
//
// The session owns its thread, not the process: stop() unsubscribes the
// thread and the host closes the process once no thread is left. The
// per-session Bridge MCP endpoint travels as the thread's `config`, so it is
// re-sent on resume.
// ============================================================================

import { randomUUID } from 'node:crypto'
import type { AgentEvent } from '../../shared/agent-events'
import type { AgentCapabilities, AgentSession, SetModelOutcome } from '../engine/agent-session'
import { EventStream } from '../engine/event-stream'
import type { CodexExit } from './app-server'
import type { TomlTable } from './config-toml'
import { CodexThreadBusyError, type CodexHost, type ThreadSink } from './codex-host'
import type { CollaborationMode } from './collaboration-modes'
import { JsonRpcError } from './jsonrpc'
import { CodexEventNormalizer } from './normalize'
import type {
  AskForApproval,
  SandboxMode,
  ThreadStartResponse,
  TurnStartResponse,
  TurnSteerResponse,
  UserInput,
} from './protocol'

/** Reasoning summary requested on every turn: what the chat shows as thinking. */
export const CODEX_REASONING_SUMMARY = 'auto' as const

export interface CodexSessionOptions {
  host: CodexHost
  id?: string
  cwd: string
  model?: string
  effort?: string
  /** Resume this thread (rollout in the same CODEX_HOME) instead of starting one. */
  resumeThreadId?: string
  /** Thread-scoped config, e.g. bridgeMcpThreadConfig(); re-sent on resume. */
  threadConfig?: TomlTable
  developerInstructions?: string
  collaborationMode?: CollaborationMode
  availableModes?: CollaborationMode[]
  approvalPolicy?: AskForApproval
  sandbox?: SandboxMode
}

const textInput = (text: string): UserInput[] => [{ type: 'text', text, text_elements: [] }]

/** Text a new turn carries in front of the user's message (Bridge session
 *  context the thread has not seen yet); `delivered` runs once codex took it. */
export interface TurnContext {
  text: string
  delivered(): void
}

export class CodexAgentSession implements AgentSession, ThreadSink {
  readonly engine = 'codex' as const
  readonly capabilities: AgentCapabilities = { nativeSteer: true, liveModelSwitch: true, normalizedTranscript: true }
  readonly events = new EventStream<AgentEvent>()
  private readonly normalizer = new CodexEventNormalizer()
  private threadId: string | null = null
  private activeTurnId: string | null = null
  /** Turns already completed, so a late turn/start response cannot revive one. */
  private readonly completed = new Set<string>()
  private nextModel: string | undefined
  private currentModel: string | null = null
  private effort: string | undefined
  private collaborationMode: CollaborationMode | undefined
  private readonly idleWaiters = new Set<() => void>()
  private detach: (() => void) | undefined
  private stopped = false
  /** Submissions go out one at a time: a second message sent while the first
   *  turn/start is in flight must steer that turn, not race a second turn. */
  private sending: Promise<void> = Promise.resolve()

  private constructor(
    readonly id: string,
    private readonly options: CodexSessionOptions,
  ) {
    this.effort = options.effort
    this.collaborationMode =
      options.collaborationMode === 'default' && !options.availableModes?.includes('default')
        ? undefined
        : options.collaborationMode
  }

  static async start(options: CodexSessionOptions): Promise<CodexAgentSession> {
    const session = new CodexAgentSession(options.id ?? randomUUID(), options)
    await session.openThread()
    return session
  }

  private async openThread(): Promise<void> {
    const o = this.options
    const common = {
      model: o.model ?? null,
      cwd: o.cwd,
      approvalPolicy: o.approvalPolicy ?? 'never',
      sandbox: o.sandbox ?? 'read-only',
      ...(o.threadConfig ? { config: o.threadConfig } : {}),
      ...(o.developerInstructions ? { developerInstructions: o.developerInstructions } : {}),
    }
    // One writer per thread: refuse before codex is asked to resume it twice.
    if (o.resumeThreadId && o.host.isAttached(o.resumeThreadId)) throw new CodexThreadBusyError(o.resumeThreadId)
    const response = o.resumeThreadId
      ? await o.host.rpc.request<ThreadStartResponse>('thread/resume', {
          threadId: o.resumeThreadId,
          ...common,
          excludeTurns: true,
        })
      : await o.host.rpc.request<ThreadStartResponse>('thread/start', common)
    this.threadId = response.thread.id
    this.currentModel = response.model ?? response.thread.model ?? null
    // No chosen effort: the thread runs on the one its config resolves (which a
    // user's codex config may set below the model's default). Every turn then
    // names it, so the chat, the model line and the engine agree.
    if (!this.effort && response.reasoningEffort) this.effort = response.reasoningEffort
    this.detach = o.host.attach(this.threadId, this)
    this.events.emit({
      kind: 'session.started',
      threadId: this.threadId,
      at: Date.now(),
      engine: 'codex',
      model: response.model ?? response.thread.model ?? null,
      resumed: Boolean(o.resumeThreadId),
    })
  }

  conversationId(): string | null {
    return this.threadId
  }

  get running(): boolean {
    return this.activeTurnId !== null
  }

  onNotification(method: string, params: unknown, emittedAtMs: unknown): void {
    const turnId = (params as { turn?: { id?: string } } | undefined)?.turn?.id
    if (method === 'turn/started' && turnId && !this.completed.has(turnId)) this.activeTurnId = turnId
    for (const event of this.normalizer.normalize(method, params, emittedAtMs)) this.events.emit(event)
    if (method === 'turn/completed' && turnId) {
      this.completed.add(turnId)
      if (turnId === this.activeTurnId) {
        this.activeTurnId = null
        this.releaseIdle()
      }
    }
  }

  onExit(exit: CodexExit): void {
    this.activeTurnId = null
    this.releaseIdle()
    this.detach = undefined
    this.events.close({
      kind: 'session.exited',
      threadId: this.threadId ?? '',
      at: Date.now(),
      code: exit.code,
      signal: exit.signal,
      ...(this.stopped ? { reason: 'stopped' } : { reason: 'codex app-server exited' }),
    })
  }

  private releaseIdle(): void {
    for (const waiter of this.idleWaiters) waiter()
    this.idleWaiters.clear()
  }

  private requireThread(): string {
    if (this.stopped || !this.threadId || !this.options.host.alive) throw new Error('codex session is not running')
    return this.threadId
  }

  /** A running turn is steered; otherwise a new turn starts. Calls are
   *  serialized, so quick successive messages become one turn plus steers. */
  send(text: string, submissionId?: string, context?: () => TurnContext | null): Promise<void> {
    const next = this.sending.then(() => this.deliver(text, submissionId, context))
    this.sending = next.catch(() => undefined)
    return next
  }

  private async deliver(text: string, submissionId?: string, context?: () => TurnContext | null): Promise<void> {
    if (this.activeTurnId) return this.steerNow(text, submissionId, context)
    const threadId = this.requireThread()
    // Only a new turn carries the context: a steer joins a turn that already has it.
    const extra = context?.() ?? null
    const requestedEffort = this.effort
    const mode = this.collaborationMode
    const model = this.currentModel
    if (mode && (!model || !this.options.availableModes?.includes(mode))) {
      throw new Error('The selected Codex collaboration mode is unavailable for this thread.')
    }
    const response = await this.options.host.rpc.request<TurnStartResponse>('turn/start', {
      threadId,
      input: extra ? [...textInput(extra.text), ...textInput(text)] : textInput(text),
      ...(submissionId ? { clientUserMessageId: submissionId } : {}),
      ...(this.nextModel ? { model: this.nextModel } : {}),
      ...(requestedEffort ? { effort: requestedEffort } : {}),
      // The bundled catalog defaults every model to `none`, so without this the
      // app-server never streams a reasoning summary and the chat shows no
      // thinking at all (the CLI only shows it when its config asks).
      summary: CODEX_REASONING_SUMMARY,
      // Null instructions select the app-server's own versioned mode prompt.
      // Use this chat's model and effort; the Plan preset otherwise suggests Medium.
      ...(mode
        ? {
            collaborationMode: {
              mode,
              settings: { model, reasoning_effort: requestedEffort ?? null, developer_instructions: null },
            },
          }
        : {}),
    })
    this.nextModel = undefined
    extra?.delivered()
    const turn = response.turn
    this.events.emit({
      kind: 'turn.requestAccepted',
      threadId,
      turnId: turn.id,
      at: Date.now(),
      requestedEffort: requestedEffort ?? null,
    })
    if (turn.status === 'inProgress' && this.activeTurnId === null && !this.completed.has(turn.id)) {
      this.activeTurnId = turn.id
    }
  }

  async steer(text: string, submissionId?: string): Promise<void> {
    return this.send(text, submissionId)
  }

  /** Compact the thread's context (`thread/compact/start`) — what codex's own
   *  `/compact` does — instead of sending the text as a model turn. Runs after
   *  the current turn ends, in order with queued messages; the compaction then
   *  runs as its own turn (turn/started … contextCompaction … turn/completed). */
  compact(): Promise<void> {
    const next = this.sending.then(async () => {
      await this.waitForIdle()
      const threadId = this.requireThread()
      await this.options.host.rpc.request('thread/compact/start', { threadId })
    })
    this.sending = next.catch(() => undefined)
    return next
  }

  private async steerNow(text: string, submissionId?: string, context?: () => TurnContext | null): Promise<void> {
    const turnId = this.activeTurnId
    if (!turnId) return this.deliver(text, submissionId, context)
    const threadId = this.requireThread()
    try {
      await this.options.host.rpc.request<TurnSteerResponse>('turn/steer', {
        threadId,
        input: textInput(text),
        expectedTurnId: turnId,
        ...(submissionId ? { clientUserMessageId: submissionId } : {}),
      })
    } catch (error) {
      // The turn ended between our check and codex's: the text starts a new one.
      if (error instanceof JsonRpcError && this.activeTurnId !== turnId)
        return this.deliver(text, submissionId, context)
      throw error
    }
  }

  async interrupt(): Promise<void> {
    const turnId = this.activeTurnId
    if (!turnId || this.stopped || !this.threadId || !this.options.host.alive) return
    await this.options.host.rpc.request('turn/interrupt', { threadId: this.threadId, turnId })
  }

  /** turn/start.model applies to that turn and all later ones; codex inserts the switch itself. */
  async setModel(model: string): Promise<SetModelOutcome> {
    if (!model) throw new Error('model required')
    this.nextModel = model
    this.currentModel = model
    return 'next-turn'
  }

  get collaboration() {
    return { selected: this.collaborationMode ?? 'default', available: this.options.availableModes ?? [] }
  }

  chooseCollaboration(mode: CollaborationMode): boolean {
    if (mode === 'default' && !this.options.availableModes?.includes(mode)) {
      this.collaborationMode = undefined
      return true
    }
    if (!this.currentModel || !this.options.availableModes?.includes(mode)) return false
    this.collaborationMode = mode
    return true
  }

  /** Reasoning effort for the next and later turns (turn/start.effort). */
  setEffort(effort: string | undefined): void {
    this.effort = effort
  }

  /** The effort the next turn runs with (sent as turn/start.effort). */
  get reasoningEffort(): string | undefined {
    return this.effort
  }

  /** Model of the thread, or the one requested for the next turn. */
  get model(): string | null {
    return this.currentModel
  }

  /** Resolves once no turn is running (immediately when idle). */
  waitForIdle(timeoutMs = 120_000): Promise<void> {
    if (!this.activeTurnId) return Promise.resolve()
    return new Promise((resolve, reject) => {
      const done = () => {
        clearTimeout(timer)
        resolve()
      }
      const timer = setTimeout(() => {
        this.idleWaiters.delete(done)
        reject(new Error(`codex turn still running after ${timeoutMs}ms`))
      }, timeoutMs)
      this.idleWaiters.add(done)
    })
  }

  /** Interrupts a running turn, unsubscribes the thread and ends the event stream. */
  async stop(reason = 'stopped'): Promise<void> {
    if (this.stopped) return
    this.stopped = true
    const host = this.options.host
    const threadId = this.threadId
    if (threadId && host.alive) {
      if (this.activeTurnId) {
        await host.rpc.request('turn/interrupt', { threadId, turnId: this.activeTurnId }).catch(() => undefined)
      }
      await host.rpc.request('thread/unsubscribe', { threadId }, { timeoutMs: 5_000 }).catch(() => undefined)
    }
    this.detach?.()
    this.detach = undefined
    this.activeTurnId = null
    this.releaseIdle()
    this.events.close({
      kind: 'session.exited',
      threadId: threadId ?? '',
      at: Date.now(),
      code: null,
      signal: null,
      reason,
    })
  }
}
