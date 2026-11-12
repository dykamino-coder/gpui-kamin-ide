// ============================================================================
// The Bridge server's single codex runtime.
//
// Owner decision: one shared ChatGPT account per Bridge server, so one
// CODEX_HOME and one `codex app-server` (CodexHostRegistry) for all codex
// chats — a single writer of auth.json cannot race a token refresh. Chats are
// threads on that process. Login is the device-code flow, started by an
// administrator from the dashboard; account state is cached here and kept
// fresh by the app-server's account notifications.
// ============================================================================

import { execFile } from 'node:child_process'
import os from 'node:os'
import { injectProxyEnv } from '../config/settings'
import { debugLog, infoLog, warnLog } from '../logging'
import { resolveCodexCommand, type CodexCommand } from './binary'
import { CodexHost, CodexHostRegistry, type RestartPolicy } from './codex-host'
import { acquireCodexHomeLock } from './codex-home-lock'
import { assertBridgeOwnedHome, buildCodexEnv, defaultCodexHome, prepareCodexHome } from './codex-home'
import { classifyCodexError, type CodexErrorInfo } from './codex-errors'
import { codexLockdownConfig, toConfigOverrides } from './config-toml'
import { buildPatchedCatalog, type ModelCatalog } from './model-catalog'
import { probeCodexNetwork } from './network-probe'
import { currentModels } from './model-list-filter'

export interface CodexAccountStatus {
  loggedIn: boolean
  email: string | null
  planType: string | null
  /** Auth mode other than ChatGPT (API key, …) is reported but not used by Bridge. */
  accountType: string | null
  /** A device-code login started from the dashboard and not finished yet. */
  pendingLogin: CodexDeviceLogin | null
  /** Last failed login or account probe, already in plain language. */
  error: CodexErrorInfo | null
  /** The account read itself failed (proxy, TLS, network): `loggedIn` is then
   *  unknown, not a sign-out, and `error` says why. */
  checkFailed?: boolean
}

export interface CodexDeviceLogin {
  loginId: string
  verificationUrl: string
  userCode: string
  startedAt: number
}

/** Whether this server can run codex at all (the image ships a pinned binary). */
export interface CodexAvailability {
  available: boolean
  /** `codex --version` of the binary Bridge launches, e.g. `0.160.1`. */
  version: string | null
  /** Plain-language reason when unavailable. */
  reason: string | null
}

const AVAILABILITY_RETRY_MS = 30_000

/** `codex-cli 0.160.1` → `0.160.1` (the raw first line if it has no version). */
export function parseCodexVersion(output: string): string | null {
  const line = output
    .split(/\r?\n/)
    .find((l) => l.trim() && !l.startsWith('WARNING'))
    ?.trim()
  if (!line) return null
  return /(\d+\.\d+\.\d+(?:[-+.][\w.]+)?)/.exec(line)?.[1] ?? line
}

/** Why codex could not be launched, for the selector, chat and dashboard. */
export function codexUnavailableReason(error: unknown): string {
  const text = error instanceof Error ? error.message : String(error)
  const code = (error as NodeJS.ErrnoException | undefined)?.code
  if (code === 'ENOENT' || /\bENOENT\b/.test(text)) return 'codex is not installed on this Bridge server'
  if (code === 'EACCES' || /\bEACCES\b/.test(text)) return 'codex on this Bridge server is not executable'
  const first = text.split(/\r?\n/).find((l) => l.trim()) ?? 'unknown error'
  return `codex does not start on this Bridge server: ${first.slice(0, 200)}`
}

export interface CodexModelOption {
  id: string
  model: string
  displayName: string
  description: string
  isDefault: boolean
  defaultReasoningEffort: string | null
  reasoningEfforts: string[]
}

export interface CodexRuntimeOptions {
  env?: NodeJS.ProcessEnv
  home?: string
  command?: CodexCommand
  /** Added to the allowlisted app-server environment (tests: fake-codex knobs). */
  extraEnv?: Record<string, string>
  /** Replaces the real app-server start (tests). */
  startHost?: (home: string) => Promise<CodexHost>
  idleCloseMs?: number
  /** Crash-restart limits of the shared app-server (tests shorten them). */
  restartPolicy?: RestartPolicy
  /** Replaces the `codex --version` probe (tests). */
  probeVersion?: () => Promise<string>
  accountTtlMs?: number
  /** Replaces the proxy/TLS probe behind a failed account read (tests). */
  probeNetwork?: (env: NodeJS.ProcessEnv) => Promise<CodexErrorInfo | null>
}

const ACCOUNT_TTL_MS = 60_000

function run(command: CodexCommand, args: string[], env: NodeJS.ProcessEnv): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile(
      command.command,
      [...command.args, ...args],
      { env, windowsHide: true, maxBuffer: 64 * 1024 * 1024, timeout: 60_000 },
      (error, stdout, stderr) => {
        if (error) reject(new Error(`${error.message}${stderr ? `\n${String(stderr).slice(-2000)}` : ''}`))
        else resolve(String(stdout))
      },
    )
  })
}

export class CodexRuntime {
  readonly home: string
  private readonly env: NodeJS.ProcessEnv
  private readonly registry: CodexHostRegistry
  private catalog: Promise<ModelCatalog> | undefined
  private account: { value: CodexAccountStatus; at: number } | undefined
  private pending: CodexDeviceLogin | null = null
  private lastError: CodexErrorInfo | null = null
  private readonly listening = new WeakSet<CodexHost>()
  private readonly accountListeners = new Set<(status: CodexAccountStatus) => void>()
  private probe: { value: Promise<CodexAvailability>; at: number; ok: boolean } | undefined

  constructor(private readonly options: CodexRuntimeOptions = {}) {
    this.env = options.env ?? process.env
    this.home = options.home ?? defaultCodexHome(this.env)
    this.registry = new CodexHostRegistry(
      (home) => (options.startHost ?? ((h) => this.startHost(h)))(home),
      options.restartPolicy,
    )
  }

  /** The shared app-server, started on first use and after it exits. */
  async host(): Promise<CodexHost> {
    const host = await this.registry.acquire(this.home)
    if (!this.listening.has(host)) {
      this.listening.add(host)
      host.onHostNotification((method, params) => this.onAccountNotification(method, params))
    }
    return host
  }

  private async startHost(home: string): Promise<CodexHost> {
    assertBridgeOwnedHome(home, os.homedir())
    const command = this.options.command ?? resolveCodexCommand({ env: this.env })
    // Same proxy / corporate CA point as the Claude CLI (injectProxyEnv).
    const env = buildCodexEnv({ home, base: this.env, injectProxy: injectProxyEnv, extra: this.options.extraEnv })
    this.catalog ??= buildPatchedCatalog((args) => run(command, args, env)).catch((error) => {
      this.catalog = undefined
      throw error
    })
    const prepared = prepareCodexHome(home, await this.catalog)
    // Never two app-servers on one thread store, even from two Bridge processes.
    const lock = acquireCodexHomeLock(home)
    infoLog('codex app-server starting', { home })
    try {
      const host = await CodexHost.start(
        {
          command,
          env,
          cwd: home,
          configOverrides: toConfigOverrides(codexLockdownConfig({ catalogPath: prepared.catalogPath })),
          onProtocolError: (error) => warnLog('codex protocol error', { error: error.message }),
        },
        { idleCloseMs: this.options.idleCloseMs },
      )
      host.onClose(() => {
        if (host.crashed) warnLog('codex app-server exited unexpectedly', { home, exit: host.process.exited })
        lock.release()
      })
      return host
    } catch (error) {
      lock.release()
      throw error
    }
  }

  // ── Availability ───────────────────────────────────────────────────────

  /** Runs `codex --version` once; a failure is re-probed after 30 s so a
   *  fixed installation is picked up without a server restart. */
  availability(options: { fresh?: boolean } = {}): Promise<CodexAvailability> {
    const cached = this.probe
    // A known-good answer is kept; a failure (or one in flight) expires.
    if (!options.fresh && cached && (cached.ok || Date.now() - cached.at < AVAILABILITY_RETRY_MS)) return cached.value
    const entry: { value: Promise<CodexAvailability>; at: number; ok: boolean } = {
      value: this.runProbe(),
      at: Date.now(),
      ok: false,
    }
    void entry.value.then((a) => {
      entry.ok = a.available
    })
    this.probe = entry
    return entry.value
  }

  private async runProbe(): Promise<CodexAvailability> {
    const probe = this.options.probeVersion
    // A test runtime with an injected app-server and no binary is available.
    if (!probe && this.options.startHost && !this.options.command)
      return { available: true, version: null, reason: null }
    try {
      const output = probe
        ? await probe()
        : await run(this.options.command ?? resolveCodexCommand({ env: this.env }), ['--version'], this.env)
      return { available: true, version: parseCodexVersion(output), reason: null }
    } catch (error) {
      const reason = codexUnavailableReason(error)
      warnLog('codex unavailable', { reason })
      return { available: false, version: null, reason }
    }
  }

  // ── Account ────────────────────────────────────────────────────────────

  onAccountChange(listener: (status: CodexAccountStatus) => void): () => void {
    this.accountListeners.add(listener)
    return () => this.accountListeners.delete(listener)
  }

  private onAccountNotification(method: string, params: unknown): void {
    if (method === 'account/login/completed') {
      const p = (params ?? {}) as { loginId?: string | null; success?: boolean; error?: string | null }
      if (!this.pending || !p.loginId || p.loginId === this.pending.loginId) this.pending = null
      this.lastError = p.success ? null : classifyCodexError(p.error ?? 'login failed')
      infoLog('codex login completed', { success: Boolean(p.success) })
    } else if (method !== 'account/updated') {
      return
    }
    this.account = undefined
    void this.accountStatus({ fresh: true })
      .then((status) => {
        for (const listener of this.accountListeners) listener(status)
      })
      .catch(() => undefined)
  }

  /** Last account state read from the app-server, without starting one; null
   *  until the first read (a Claude-only server never starts codex for it). */
  knownAccount(): CodexAccountStatus | null {
    return this.account ? { ...this.account.value, pendingLogin: this.pending, error: this.lastError } : null
  }

  async accountStatus(options: { fresh?: boolean } = {}): Promise<CodexAccountStatus> {
    const ttl = this.options.accountTtlMs ?? ACCOUNT_TTL_MS
    if (!options.fresh && this.account && Date.now() - this.account.at < ttl) {
      return { ...this.account.value, pendingLogin: this.pending, error: this.lastError }
    }
    try {
      const host = await this.host()
      const res = (await host.rpc.request('account/read', { refreshToken: false })) as {
        account: { type: string; email?: string | null; planType?: string | null } | null
      }
      const a = res.account
      const value: CodexAccountStatus = {
        loggedIn: a?.type === 'chatgpt',
        email: a?.email ?? null,
        planType: a?.planType ?? null,
        accountType: a?.type ?? null,
        pendingLogin: this.pending,
        error: this.lastError,
      }
      this.account = { value, at: Date.now() }
      return value
    } catch (error) {
      return {
        loggedIn: false,
        email: null,
        planType: null,
        accountType: null,
        pendingLogin: this.pending,
        error: await this.explainReadFailure(classifyCodexError((error as Error).message)),
        checkFailed: true,
      }
    }
  }

  /** codex names no cause for a blocked connection ("workspace routing
   *  discovery failed"); ask the network the way codex would. */
  private async explainReadFailure(info: CodexErrorInfo): Promise<CodexErrorInfo> {
    if (info.kind !== 'other' && info.kind !== 'network') return info
    const env = buildCodexEnv({ home: this.home, base: this.env, injectProxy: injectProxyEnv })
    const cause = await (this.options.probeNetwork ?? probeCodexNetwork)(env).catch(() => null)
    if (!cause) return info
    return { kind: cause.kind, message: cause.message, detail: `${info.detail}; ${cause.detail}` }
  }

  /** Starts the ChatGPT device-code login; the admin opens the URL and enters the code. */
  async startDeviceLogin(): Promise<CodexDeviceLogin> {
    const host = await this.host()
    if (this.pending) {
      await host.rpc.request('account/login/cancel', { loginId: this.pending.loginId }).catch(() => undefined)
    }
    const res = (await host.rpc.request('account/login/start', { type: 'chatgptDeviceCode' })) as {
      type: string
      loginId: string
      verificationUrl: string
      userCode: string
    }
    if (res.type !== 'chatgptDeviceCode') throw new Error(`unexpected login response ${res.type}`)
    this.lastError = null
    this.pending = {
      loginId: res.loginId,
      verificationUrl: res.verificationUrl,
      userCode: res.userCode,
      startedAt: Date.now(),
    }
    debugLog('codex device login started', { loginId: res.loginId })
    return this.pending
  }

  async cancelLogin(): Promise<void> {
    const pending = this.pending
    this.pending = null
    if (!pending) return
    const host = await this.host()
    await host.rpc.request('account/login/cancel', { loginId: pending.loginId }).catch(() => undefined)
  }

  async logout(): Promise<void> {
    const host = await this.host()
    await host.rpc.request('account/logout')
    this.account = {
      value: { loggedIn: false, email: null, planType: null, accountType: null, pendingLogin: null, error: null },
      at: Date.now(),
    }
    this.pending = null
    this.lastError = null
  }

  // ── Models ─────────────────────────────────────────────────────────────

  async models(): Promise<CodexModelOption[]> {
    const host = await this.host()
    const out: Array<
      CodexModelOption & { hidden: boolean; upgrade: string | null; upgradeInfo: { model?: string | null } | null }
    > = []
    let cursor: string | null = null
    for (let page = 0; page < 20; page++) {
      const res = (await host.rpc.request('model/list', { cursor, includeHidden: false })) as {
        data: Array<{
          id: string
          model: string
          displayName: string
          description: string
          hidden: boolean
          upgrade?: string | null
          upgradeInfo?: { model?: string | null } | null
          isDefault: boolean
          defaultReasoningEffort?: string | null
          supportedReasoningEfforts?: Array<{ reasoningEffort?: string } | string>
        }>
        nextCursor: string | null
      }
      for (const m of res.data) {
        out.push({
          hidden: m.hidden,
          upgrade: m.upgrade ?? null,
          upgradeInfo: m.upgradeInfo ?? null,
          id: m.id,
          model: m.model,
          displayName: m.displayName,
          description: m.description,
          isDefault: m.isDefault,
          defaultReasoningEffort: m.defaultReasoningEffort ?? null,
          reasoningEfforts: (m.supportedReasoningEfforts ?? [])
            .map((e) => (typeof e === 'string' ? e : e.reasoningEffort))
            .filter((e): e is string => typeof e === 'string'),
        })
      }
      cursor = res.nextCursor
      if (!cursor) break
    }
    // Current generation only (model-list-filter); the selector and model
    // changes all read this list.
    return currentModels(out).map(({ hidden: _hidden, upgrade: _upgrade, upgradeInfo: _info, ...option }) => option)
  }

  async close(): Promise<void> {
    await this.registry.closeAll()
  }
}

let shared: CodexRuntime | undefined

export function codexRuntime(): CodexRuntime {
  return (shared ??= new CodexRuntime())
}

/** Tests install their own runtime (fake codex, temp home). */
export function setCodexRuntimeForTests(runtime: CodexRuntime | undefined): void {
  shared = runtime
}

/** Server shutdown: closes the shared app-server if it was ever started. */
export async function shutdownCodexRuntime(): Promise<void> {
  const runtime = shared
  if (runtime) await runtime.close().catch(() => undefined)
}

/** codex in this image: the version Bridge launches and the image pin. Never
 *  blocks the status page for long — a slow first probe reports as pending. */
export async function codexStatus(
  runtime: () => CodexRuntime = codexRuntime,
  env: NodeJS.ProcessEnv = process.env,
  waitMs = 3_000,
): Promise<CodexAvailability & { pinned: string | null }> {
  const pinned = env.BRIDGE_CODEX_VERSION || null
  const pending: CodexAvailability = { available: false, version: null, reason: 'checking codex…' }
  let timer: ReturnType<typeof setTimeout> | undefined
  const timeout = new Promise<CodexAvailability>((resolve) => {
    timer = setTimeout(() => resolve(pending), waitMs)
  })
  try {
    return { ...(await Promise.race([runtime().availability(), timeout])), pinned }
  } finally {
    clearTimeout(timer)
  }
}
