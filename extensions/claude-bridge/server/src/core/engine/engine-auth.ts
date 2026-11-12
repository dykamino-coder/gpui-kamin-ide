// Which agent engines this Bridge server is signed in to.
//
// The model selector disables the models of an engine the server cannot run
// (no sign-in, or no codex binary), refuses it as the main agent, and a new
// chat whose engine is signed out opens on the signed-in one instead. Clients
// get the state with `agent:preferences` and a fresh copy after every sign-in
// or sign-out (`claude:account-updated` / `codex:account-updated`).
//
// `loggedIn: null` means "not known yet": nothing is disabled on a guess. A
// failed `claude auth status` (timeout, missing CLI) is unknown, not signed out.
import fs from 'fs'
import os from 'os'
import path from 'path'
import type { AgentEngineId } from '../../shared/agent-events'
import type { CodexRuntime } from '../codex/codex-runtime'
import { eventBus } from '../events/bus'
import { claudeInvocation } from '../pty/dev-cli'
import { debugLog } from '../logging'

export interface EngineAuth {
  loggedIn: boolean | null
  /** Why the engine cannot run chats; null when it can (or is unknown). */
  reason: string | null
}

export type EngineAuthState = Record<AgentEngineId, EngineAuth>

export interface ClaudeAuthResult {
  loggedIn: boolean
  authMethod?: string
  apiProvider?: string
  error?: string
}

export const CLAUDE_SIGNED_OUT = 'Claude is not signed in on this Bridge server.'
export const CODEX_SIGNED_OUT = 'ChatGPT is not signed in on this Bridge server.'
const ENGINE_NAME: Record<AgentEngineId, string> = { claude: 'Claude', codex: 'ChatGPT' }

/** Runs `claude auth status`. Throws when the CLI does not answer: that is an
 *  unknown state, not a sign-out. */
export async function runClaudeAuthStatus(): Promise<ClaudeAuthResult> {
  const { execFile } = await import('child_process')
  const { promisify } = await import('util')
  const cli = claudeInvocation(['--dangerously-skip-permissions', 'auth', 'status'])
  const { stdout, stderr } = await promisify(execFile)(cli.command, cli.args, {
    timeout: 10_000,
    env: { ...process.env, HOME: process.env.HOME || '/home/bridge' },
  })
  const trimmed = stdout.trim()
  if (!trimmed) throw new Error(`Empty stdout from \`claude auth status\`. stderr: ${(stderr || '').slice(0, 200)}`)
  let parsed: Record<string, unknown>
  try {
    parsed = JSON.parse(trimmed) as Record<string, unknown>
  } catch {
    throw new Error(`claude auth status returned non-JSON: ${trimmed.slice(0, 200)}`)
  }
  return {
    loggedIn: parsed.loggedIn === true,
    authMethod: typeof parsed.authMethod === 'string' ? parsed.authMethod : undefined,
    apiProvider: typeof parsed.apiProvider === 'string' ? parsed.apiProvider : undefined,
  }
}

export interface ClaudeAuthMonitorOptions {
  probe?: () => Promise<ClaudeAuthResult>
  now?: () => number
  ttlMs?: number
  /** Called when the known sign-in state changes (first answer included). */
  onChange?: (loggedIn: boolean) => void
}

/** Claude sign-in state of this server, cached for two minutes and refreshed
 *  at once when the credentials file changes (`claude login` / `logout`). */
export class ClaudeAuthMonitor {
  private last: { value: ClaudeAuthResult; at: number } | null = null
  private known: boolean | null = null
  private inflight: Promise<ClaudeAuthResult> | null = null
  private watcher: fs.FSWatcher | null = null
  private debounce: ReturnType<typeof setTimeout> | null = null

  constructor(private readonly options: ClaudeAuthMonitorOptions = {}) {}

  /** Sign-in state without running the CLI; null until the first answer. */
  loggedIn(): boolean | null {
    return this.known
  }

  /** Cached `claude auth status`; rejects when the CLI does not answer. */
  status(options: { fresh?: boolean } = {}): Promise<ClaudeAuthResult> {
    const now = this.options.now ?? Date.now
    const ttl = this.options.ttlMs ?? 2 * 60_000
    if (!options.fresh && this.last && now() - this.last.at < ttl) return Promise.resolve(this.last.value)
    if (this.inflight) return this.inflight
    const run = (this.options.probe ?? runClaudeAuthStatus)()
    this.inflight = run.then(
      (value) => {
        this.inflight = null
        this.record(value)
        return value
      },
      (error: unknown) => {
        this.inflight = null
        throw error
      },
    )
    return this.inflight
  }

  /** The state; a known one answers at once (a stale one is refreshed in the
   *  background), an unknown one waits at most `waitMs` for the CLI. */
  async loggedInWithin(waitMs: number): Promise<boolean | null> {
    if (this.known !== null) {
      void this.status().catch(() => undefined)
      return this.known
    }
    const timeout = new Promise<null>((resolve) => {
      const timer: ReturnType<typeof setTimeout> = setTimeout(() => resolve(null), waitMs)
      timer.unref?.()
    })
    try {
      const value = await Promise.race([this.status(), timeout])
      return value ? value.loggedIn : this.known
    } catch {
      return this.known
    }
  }

  /** A sign-in seen elsewhere (startup or dashboard health check). */
  record(value: ClaudeAuthResult): void {
    const now = this.options.now ?? Date.now
    this.last = { value, at: now() }
    if (this.known === value.loggedIn) return
    this.known = value.loggedIn
    this.options.onChange?.(value.loggedIn)
  }

  /** Refreshes on any change of `~/.claude/.credentials.json`. */
  watch(home: string = process.env.HOME || os.homedir()): void {
    if (this.watcher) return
    const dir = path.join(home, '.claude')
    try {
      this.watcher = fs.watch(dir, (_event, name) => {
        if (name && String(name) !== '.credentials.json') return
        if (this.debounce) clearTimeout(this.debounce)
        this.debounce = setTimeout(() => {
          this.debounce = null
          void this.status({ fresh: true }).catch(() => undefined)
        }, 500)
        this.debounce.unref?.()
      })
      this.watcher.unref?.()
      this.watcher.on('error', () => this.unwatch())
    } catch (error) {
      debugLog('claude credentials watch unavailable', { dir, error: (error as Error).message })
    }
  }

  unwatch(): void {
    this.watcher?.close()
    this.watcher = null
    if (this.debounce) clearTimeout(this.debounce)
    this.debounce = null
  }
}

let shared: ClaudeAuthMonitor | null = null

/** The server's Claude sign-in monitor; a change is announced on the event bus. */
export function claudeAuth(): ClaudeAuthMonitor {
  if (!shared) {
    shared = new ClaudeAuthMonitor({
      onChange: (loggedIn) => eventBus.emit('claude:account-updated', { engine: 'claude', loggedIn }),
    })
    const monitor = shared
    // Startup and dashboard health checks already run `claude auth status`.
    eventBus.on('health:updated', (event) => {
      const sdk = (event.data as { sdk?: unknown } | null)?.sdk
      if (sdk === true) monitor.record({ loggedIn: true })
      else if (sdk === false) void monitor.status({ fresh: true }).catch(() => undefined)
    })
    if (process.env.VITEST !== 'true') monitor.watch()
  }
  return shared
}

/** Tests install their own monitor (a scripted `claude auth status`). */
export function setClaudeAuthForTests(monitor: ClaudeAuthMonitor | null): void {
  shared = monitor
}

export function claudeEngineAuth(loggedIn: boolean | null): EngineAuth {
  return { loggedIn, reason: loggedIn === false ? CLAUDE_SIGNED_OUT : null }
}

/** ChatGPT state. `probe: false` reads only what is already known and never
 *  starts an app-server; `true` asks it (starting one if needed). */
export async function codexEngineAuth(runtime: CodexRuntime, probe: boolean): Promise<EngineAuth> {
  const availability = await runtime.availability()
  if (!availability.available) {
    return { loggedIn: false, reason: `ChatGPT is unavailable: ${availability.reason ?? 'codex does not start'}` }
  }
  const account = probe ? await runtime.accountStatus() : runtime.knownAccount()
  if (!account) return { loggedIn: null, reason: null }
  if (account.loggedIn) return { loggedIn: true, reason: null }
  return { loggedIn: false, reason: account.error?.message ?? CODEX_SIGNED_OUT }
}

export interface EngineAuthDeps {
  claude?: ClaudeAuthMonitor
  runtime: CodexRuntime
}

/** Both engines; `probe` asks codex (see codexEngineAuth), Claude is waited for briefly. */
export async function engineAuthState(deps: EngineAuthDeps, probe = false, waitMs = 2000): Promise<EngineAuthState> {
  const claude = deps.claude ?? claudeAuth()
  const [claudeLoggedIn, codex] = await Promise.all([
    claude.loggedInWithin(waitMs),
    codexEngineAuth(deps.runtime, probe).catch(() => ({ loggedIn: null, reason: null }) as EngineAuth),
  ])
  return { claude: claudeEngineAuth(claudeLoggedIn), codex }
}

async function engineState(engine: AgentEngineId, deps: EngineAuthDeps, waitMs: number): Promise<EngineAuth> {
  if (engine === 'claude') return claudeEngineAuth(await (deps.claude ?? claudeAuth()).loggedInWithin(waitMs))
  return codexEngineAuth(deps.runtime, true).catch(() => ({ loggedIn: null, reason: null }))
}

export interface EngineChoice {
  engine: AgentEngineId
  /** Told to the user when the chat runs on another engine than asked. */
  notice: string | null
}

/** Engine for a brand-new chat: the requested one, or the other engine when
 *  the requested one is signed out and the other is signed in. With both
 *  signed out (or unknown) the request stands and fails with its own reason. */
export async function chooseEngineForNewChat(
  requested: AgentEngineId,
  deps: EngineAuthDeps,
  waitMs = 3000,
): Promise<EngineChoice> {
  const own = await engineState(requested, deps, waitMs)
  if (own.loggedIn !== false) return { engine: requested, notice: null }
  const other: AgentEngineId = requested === 'claude' ? 'codex' : 'claude'
  const alt = await engineState(other, deps, waitMs)
  if (alt.loggedIn !== true) return { engine: requested, notice: null }
  return {
    engine: other,
    notice: `${own.reason ?? `${ENGINE_NAME[requested]} is not signed in.`} This chat runs ${ENGINE_NAME[other]} instead; sign ${ENGINE_NAME[requested]} in from the Bridge dashboard.`,
  }
}

/** Why `engine` cannot be the main agent, or null when it can. */
export async function mainAgentRefusal(engine: AgentEngineId, deps: EngineAuthDeps): Promise<string | null> {
  const state = await engineState(engine, deps, 3000)
  return state.loggedIn === false ? state.reason : null
}
