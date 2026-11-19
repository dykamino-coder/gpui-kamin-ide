// Per-process private UDS transport. A claimed prompt is never silently replayed.
import http from 'node:http'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFile } from 'node:child_process'
import type { CliHandlerConditions } from '../hooks/cli-handler-conditions'

type Command =
  | { id: number; kind: 'submit'; text: string; submissionId?: string }
  | { id: number; kind: 'abort'; turnId: string }
  | { id: number; kind: 'arm'; submissionId: string }
type Pending = { command: Command; claimed: boolean; fallback: () => void; accepted?: () => void }
export const CLI_MOD_PLUGIN = fileURLToPath(new URL('../../cli-mod', import.meta.url))
/** The module files of the entry with hook handler conditions (hooks/bridge-conditions.js and what it imports). */
const CONDITION_MODULES = ['bridge-conditions.js', 'conditions.js', 'transport.js']
export const CLI_MOD_VERSION = '2.1.327'
/** How long an idle /poll is held open. Below the 3 s lease, so a module that
 * stops polling still expires on time. */
export const POLL_HOLD_MS = 2000

export class CliModTransport {
  readonly directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-mod-'))
  readonly socketPath = path.join(this.directory, 'cli.sock')
  private server: http.Server
  private instance = ''
  private heartbeat = 0
  private sequence = 0
  private pending = new Map<number, Pending>()
  private closed = false
  private turnId: string | null = null
  private held: { res: http.ServerResponse; timer: ReturnType<typeof setTimeout> } | undefined
  private timer: ReturnType<typeof setInterval>
  onTurn: (turnId: string | null) => void = () => {}
  onUnavailable: () => void = () => {}
  onUncertain: () => void = () => {}
  hookConditions?: CliHandlerConditions

  constructor(readonly sessionId: string) {
    fs.chmodSync(this.directory, 0o700)
    this.server = http.createServer((req, res) => {
      let body = ''
      req.setEncoding('utf8')
      req.on('data', (chunk: string) => {
        body += chunk
        if (Buffer.byteLength(body) > 1_048_576) req.destroy()
      })
      req.on('error', () => {})
      req.on('end', async () => {
        try {
          const data = JSON.parse(body)
          if (
            req.method !== 'POST' ||
            data.protocol !== 1 ||
            data.session !== this.sessionId ||
            data.version !== CLI_MOD_VERSION ||
            typeof data.instance !== 'string' ||
            !data.instance ||
            data.instance.length > 128
          ) {
            res.writeHead(409).end()
            return
          }
          if (req.url === '/hello') {
            if (this.instance && this.instance !== data.instance) this.expire()
            this.instance = data.instance
          } else if (
            !['/poll', '/hooks/prepare', '/hooks/restore', '/hooks/release', '/hooks/wait'].includes(req.url ?? '') ||
            !this.instance ||
            this.instance !== data.instance
          ) {
            res.writeHead(409).end()
            return
          }
          this.heartbeat = Date.now()
          if (req.url?.startsWith('/hooks/')) {
            if (!this.hookConditions) {
              res.writeHead(409).end()
              return
            }
            let result: { generation?: string } = {}
            if (req.url === '/hooks/wait') await new Promise((resolve) => setTimeout(resolve, 50))
            else if (typeof data.generation !== 'string') {
              res.writeHead(400).end()
              return
            } else if (req.url === '/hooks/prepare') {
              const controller = new AbortController()
              res.once('close', () => {
                if (!res.writableEnded) controller.abort()
              })
              result = await this.hookConditions.prepare(data.event, data.payload, data.generation, controller.signal)
            } else if (req.url === '/hooks/restore') result = this.hookConditions.restore(data.generation)
            else this.hookConditions.release(data.generation)
            res.writeHead(200, { 'Content-Type': 'application/json' }).end(JSON.stringify(result))
            return
          }
          if (Array.isArray(data.turns)) {
            for (const turn of data.turns.slice(0, 256)) {
              if (turn.turnId === null || (typeof turn.turnId === 'string' && turn.turnId.length < 256)) {
                this.recordTurn(turn.turnId)
              }
            }
          }
          if (data.turnId === null || (typeof data.turnId === 'string' && data.turnId.length < 256)) {
            this.recordTurn(data.turnId)
          }
          if (Array.isArray(data.acks)) {
            for (const ack of data.acks.slice(0, 128)) {
              const pending = this.pending.get(ack.id)
              if (!pending?.claimed) continue
              this.pending.delete(ack.id)
              // A rejected submit can have passed another plugin's gate; do not replay it.
              if (ack.rejected) this.onUncertain()
              else pending.accepted?.()
            }
          }
          const commands = this.takeCommands()
          if (!commands.length && data.wait === true && !this.held) {
            this.hold(res)
            return
          }
          res.writeHead(200, { 'Content-Type': 'application/json' }).end(JSON.stringify({ commands }))
        } catch {
          res.writeHead(400).end()
        }
      })
    })
    this.server.requestTimeout = 2000
    this.server.headersTimeout = 2000
    this.timer = setInterval(() => this.healthy(), 500)
    this.timer.unref()
  }

  async listen(): Promise<void> {
    await new Promise<void>((resolve, reject) => {
      this.server.once('error', reject)
      this.server.listen(this.socketPath, () => {
        fs.chmodSync(this.socketPath, 0o600)
        resolve()
      })
    })
  }

  private created: string[] = []

  /** Plugin root for a session with hook handler conditions. The CLI dispatches
   * every tool call through each handler a module registers, so the default
   * root loads only the transport; this one, in the private directory, loads the
   * same plugin with the condition gates added. */
  conditionsPlugin(): string {
    const root = path.join(this.directory, 'plugin')
    if (this.created.length) return root
    const write = (relative: string, data: string | Buffer) => {
      const file = path.join(root, relative)
      fs.mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 })
      fs.writeFileSync(file, data, { mode: 0o600, flag: 'wx' })
      this.created.push(file)
    }
    write('.claude-plugin/plugin.json', fs.readFileSync(path.join(CLI_MOD_PLUGIN, '.claude-plugin', 'plugin.json')))
    write('hooks/hooks.json', JSON.stringify({ modules: ['./bridge-conditions.js'] }))
    for (const name of CONDITION_MODULES)
      write(`hooks/${name}`, fs.readFileSync(path.join(CLI_MOD_PLUGIN, 'hooks', name)))
    return root
  }

  private takeCommands(): Command[] {
    const commands: Command[] = []
    for (const pending of this.pending.values()) {
      // An arm only tags the next serialized PTY transaction. Until its
      // ack reaches us no text has been written, so repeating the same
      // command/id repairs a lost poll response without replaying input.
      if (!pending.claimed || pending.command.kind === 'arm') {
        pending.claimed = true
        commands.push(pending.command)
      }
    }
    return commands
  }

  /** Long poll: an idle module waits here instead of asking ten times a second.
   * The held request is the heartbeat; a new command answers it at once. */
  private hold(res: http.ServerResponse): void {
    const held = { res, timer: setTimeout(() => this.answerHeld(), POLL_HOLD_MS) }
    this.held = held
    res.once('close', () => {
      if (this.held !== held) return
      clearTimeout(held.timer)
      this.held = undefined
      if (!res.writableEnded) this.heartbeat = Date.now()
    })
  }

  /** claim: false answers empty: teardown must not hand out commands it is about to fall back on. */
  private answerHeld(claim = true): void {
    const held = this.held
    if (!held) return
    this.held = undefined
    clearTimeout(held.timer)
    this.heartbeat = Date.now()
    const commands = claim && !this.closed ? this.takeCommands() : []
    held.res.writeHead(200, { 'Content-Type': 'application/json' }).end(JSON.stringify({ commands }))
  }

  healthy(): boolean {
    if (this.closed || !this.heartbeat) return false
    if (this.held) return true
    if (Date.now() - this.heartbeat <= 3000) return true
    this.expire()
    return false
  }

  private expire(): void {
    this.answerHeld(false)
    this.heartbeat = 0
    this.instance = ''
    this.turnId = null
    this.resetHookConditions()
    const pending = [...this.pending.values()]
    this.pending.clear()
    this.onUnavailable()
    for (const item of pending) {
      if (!item.claimed) item.fallback()
      else this.onUncertain()
    }
  }

  private recordTurn(turnId: string | null): void {
    if (this.turnId !== null && turnId === null) this.resetHookConditions()
    this.turnId = turnId
    this.onTurn(turnId)
  }

  private resetHookConditions(): void {
    try {
      this.hookConditions?.reset()
    } catch {
      // Teardown can remove the conversation directory before the last poll.
    }
  }

  submit(text: string, fallback: () => void, submissionId?: string, accepted?: () => void): boolean {
    if (!this.healthy() || this.pending.size >= 64 || Buffer.byteLength(text) > 131_072) return false
    const command: Command = { id: ++this.sequence, kind: 'submit', text, ...(submissionId ? { submissionId } : {}) }
    this.pending.set(command.id, { command, claimed: false, fallback, accepted })
    this.answerHeld()
    return true
  }

  arm(submissionId: string, accepted: () => void, fallback: () => void): boolean {
    if (!this.healthy() || this.pending.size >= 64) return false
    const command: Command = { id: ++this.sequence, kind: 'arm', submissionId }
    this.pending.set(command.id, { command, claimed: false, fallback, accepted })
    this.answerHeld()
    return true
  }

  abort(turnId: string, fallback: () => void): boolean {
    if (!this.healthy() || this.pending.size >= 64) return false
    const command: Command = { id: ++this.sequence, kind: 'abort', turnId }
    this.pending.set(command.id, { command, claimed: false, fallback })
    this.answerHeld()
    return true
  }

  /** Withdraw only commands that provably have not delivered text. An arm
   * carries no text even when claimed; its callback has not run until ack. */
  withdrawSubmission(submissionId: string): boolean {
    for (const [id, pending] of this.pending) {
      const command = pending.command
      if (command.kind === 'abort' || command.submissionId !== submissionId) continue
      if (command.kind === 'submit' && pending.claimed) return false
      this.pending.delete(id)
      return true
    }
    return false
  }

  dispose(): void {
    if (this.closed) return
    this.closed = true
    clearInterval(this.timer)
    this.answerHeld(false)
    this.pending.clear()
    try {
      this.hookConditions?.dispose()
    } catch {
      // The conversation directory can already have been removed by teardown.
    }
    this.server.close()
    this.server.closeAllConnections()
    // Only paths created by this instance; never recursively remove a session directory.
    try {
      fs.unlinkSync(this.socketPath)
    } catch {
      /* already closed */
    }
    for (const file of this.created.splice(0)) {
      try {
        fs.unlinkSync(file)
      } catch {
        /* already removed */
      }
    }
    const root = path.join(this.directory, 'plugin')
    for (const dir of [path.join(root, 'hooks'), path.join(root, '.claude-plugin'), root]) {
      try {
        fs.rmdirSync(dir)
      } catch {
        /* not created */
      }
    }
    try {
      fs.rmdirSync(this.directory)
    } catch {
      /* listen failed or still closing */
    }
  }
}

/** How long one `--version` answer stands for every session start. */
export const CLI_VERSION_PROBE_TTL_MS = 60_000
const versionProbes = new Map<string, { at: number; version: Promise<string> }>()

/** Tests: forget shared `--version` answers between cases. */
export function resetCliVersionProbes(): void {
  versionProbes.clear()
}

/** `claude --version`, shared by concurrent session starts and reused for a
 *  minute: spawning the CLI once per new chat cost the main thread ~10 ms each,
 *  two seconds when 200 chats resumed at once. Safe to reuse because the mod
 *  handshake checks the version again in the spawned CLI. */
export function probeCliVersion(
  cli: { command: string; prefix: string[]; env: NodeJS.ProcessEnv },
  now = Date.now(),
): Promise<string> {
  const key = [cli.command, ...cli.prefix].join('\0')
  const cached = versionProbes.get(key)
  if (cached && now - cached.at < CLI_VERSION_PROBE_TTL_MS) return cached.version
  const version = new Promise<string>((resolve) => {
    try {
      execFile(
        cli.command,
        [...cli.prefix, '--version'],
        { env: cli.env, timeout: 5000, maxBuffer: 65536 },
        (error, stdout) => {
          resolve(error ? '' : stdout.trim())
        },
      )
    } catch {
      // A synchronous spawn failure is a failed probe, not a rejection.
      resolve('')
    }
  })
  versionProbes.set(key, { at: now, version })
  // A failed probe is not remembered: the next start asks again.
  void version.then((v) => {
    if (!v && versionProbes.get(key)?.version === version) versionProbes.delete(key)
  })
  return version
}

export async function createCliModTransport(
  sessionId: string,
  cli = { command: 'claude', prefix: [] as string[], env: process.env },
): Promise<CliModTransport | undefined> {
  if (process.env.BRIDGE_CLI_MOD === '0' || process.platform !== 'linux') return undefined
  // Do not load version-specific hooks into an unknown CLI. The handshake also
  // checks the version in case the executable changes between probe and spawn.
  let transport: CliModTransport | undefined
  try {
    const version = await probeCliVersion(cli)
    if (version !== `${CLI_MOD_VERSION} (Claude Code)`) return undefined
    transport = new CliModTransport(sessionId)
    await transport.listen()
    return transport
  } catch {
    transport?.dispose()
    return undefined
  }
}
