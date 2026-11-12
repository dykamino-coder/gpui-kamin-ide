// ============================================================================
// One `codex app-server` child per CODEX_HOME (one per Bridge server, shared
// by every codex session as threads — see codex-host.ts), spoken to over stdio.
//
// Lifecycle: spawn → `initialize` (experimentalApi: thread/turn steering and
// the item stream Bridge renders are experimental in 0.160.1) → `initialized`
// → … → close. Closing ends stdin first: the npm launcher passes the inherited
// stdin to the native binary, which exits on EOF, so the graceful path leaves
// no orphan. Only after the grace period is the whole process tree killed.
// ============================================================================

import { spawn as nodeSpawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import type { CodexCommand } from './binary'
import { JsonRpcClosedError, JsonRpcConnection } from './jsonrpc'
import type { ClientInfo, InitializeParams, InitializeResponse } from './protocol'

export interface CodexExit {
  code: number | null
  signal: NodeJS.Signals | null
}

export type SpawnFn = (
  command: string,
  args: string[],
  options: { cwd: string; env: NodeJS.ProcessEnv; detached: boolean; windowsHide: boolean },
) => ChildProcessWithoutNullStreams

export interface CodexAppServerOptions {
  command: CodexCommand
  env: NodeJS.ProcessEnv
  cwd: string
  /** `-c key=value` pairs from toConfigOverrides(). */
  configOverrides?: string[]
  clientInfo?: ClientInfo
  requestTimeoutMs?: number
  initializeTimeoutMs?: number
  stderrLimitBytes?: number
  spawn?: SpawnFn
  /** Kills the process tree; injectable for tests. */
  killTree?: (child: ChildProcessWithoutNullStreams) => void
  onProtocolError?: (error: Error) => void
  onTrace?: (direction: 'in' | 'out', message: Record<string, unknown>) => void
}

export const BRIDGE_CLIENT_INFO: ClientInfo = { name: 'kamin_bridge', title: 'KaminIDE Bridge', version: '1' }

function defaultKillTree(child: ChildProcessWithoutNullStreams): void {
  if (child.pid === undefined) return
  if (process.platform === 'win32') {
    // TerminateProcess on the Node launcher would orphan the native codex.
    nodeSpawn('taskkill', ['/pid', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' }).on(
      'error',
      () => child.kill(),
    )
    return
  }
  try {
    process.kill(-child.pid, 'SIGKILL') // detached → own process group
  } catch {
    child.kill('SIGKILL')
  }
}

export class CodexAppServer {
  readonly rpc: JsonRpcConnection
  private exit: CodexExit | undefined
  private readonly exitWaiters = new Set<(exit: CodexExit) => void>()
  private stderr = ''
  private readonly stderrLimit: number
  private closing: Promise<CodexExit> | undefined
  initializeResult: InitializeResponse | undefined

  private constructor(
    private readonly child: ChildProcessWithoutNullStreams,
    private readonly options: CodexAppServerOptions,
  ) {
    this.stderrLimit = options.stderrLimitBytes ?? 16 * 1024
    this.rpc = new JsonRpcConnection(child.stdout, child.stdin, {
      defaultTimeoutMs: options.requestTimeoutMs ?? 30_000,
      onProtocolError: options.onProtocolError,
      onTrace: options.onTrace,
    })
    child.stderr.setEncoding('utf8')
    child.stderr.on('data', (text: string) => {
      this.stderr = (this.stderr + text).slice(-this.stderrLimit)
    })
    // EPIPE after the child died is reported through the exit path below.
    child.stdin.on('error', () => {})
    child.on('exit', (code, signal) => {
      this.exit = { code, signal }
      this.rpc.close(new JsonRpcClosedError(`codex app-server exited (code ${code}, signal ${signal})`))
      for (const waiter of this.exitWaiters) waiter(this.exit)
      this.exitWaiters.clear()
    })
  }

  /** Spawns and completes the initialize handshake, or fails with the stderr tail. */
  static async start(options: CodexAppServerOptions): Promise<CodexAppServer> {
    const spawn = options.spawn ?? (nodeSpawn as unknown as SpawnFn)
    const args = [...options.command.args, 'app-server', ...(options.configOverrides ?? [])]
    const child = spawn(options.command.command, args, {
      cwd: options.cwd,
      env: options.env,
      detached: process.platform !== 'win32',
      windowsHide: true,
    })
    const spawnError = new Promise<never>((_, reject) => child.once('error', reject))
    spawnError.catch(() => {})
    const server = new CodexAppServer(child, options)
    try {
      await Promise.race([server.initialize(), spawnError])
      return server
    } catch (error) {
      await server.close(500).catch(() => undefined)
      const tail = server.stderrTail().trim()
      const message = (error as Error).message + (tail ? `\ncodex stderr:\n${tail.slice(-2000)}` : '')
      throw new Error(`codex app-server failed to start: ${message}`, { cause: error })
    }
  }

  private async initialize(): Promise<void> {
    const params: InitializeParams = {
      clientInfo: this.options.clientInfo ?? BRIDGE_CLIENT_INFO,
      capabilities: { experimentalApi: true, requestAttestation: false },
    }
    this.initializeResult = await this.rpc.request<InitializeResponse>('initialize', params, {
      timeoutMs: this.options.initializeTimeoutMs ?? 20_000,
    })
    this.rpc.notify('initialized')
  }

  get pid(): number | undefined {
    return this.child.pid
  }

  get exited(): CodexExit | undefined {
    return this.exit
  }

  stderrTail(): string {
    return this.stderr
  }

  onExit(listener: (exit: CodexExit) => void): () => void {
    if (this.exit) {
      listener(this.exit)
      return () => {}
    }
    this.exitWaiters.add(listener)
    return () => this.exitWaiters.delete(listener)
  }

  private waitExit(ms: number): Promise<CodexExit | undefined> {
    if (this.exit) return Promise.resolve(this.exit)
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        off()
        resolve(undefined)
      }, ms)
      const off = this.onExit((exit) => {
        clearTimeout(timer)
        resolve(exit)
      })
    })
  }

  /** Idempotent: EOF on stdin, then a tree kill after `graceMs`. */
  close(graceMs = 3_000): Promise<CodexExit> {
    if (!this.closing) {
      this.closing = (async () => {
        this.rpc.close(new JsonRpcClosedError('Bridge closed the codex session'))
        if (!this.exit) this.child.stdin.end()
        const graceful = await this.waitExit(graceMs)
        if (graceful) return graceful
        ;(this.options.killTree ?? defaultKillTree)(this.child)
        return (await this.waitExit(5_000)) ?? { code: null, signal: 'SIGKILL' }
      })()
    }
    return this.closing
  }
}
