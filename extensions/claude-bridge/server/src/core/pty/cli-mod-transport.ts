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
export const CLI_MOD_VERSION = '2.1.322'

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
  private timer: ReturnType<typeof setInterval>
  onTurn: (turnId: string | null) => void = () => {}
  onUnavailable: () => void = () => {}
  onUncertain: () => void = () => {}
  onRead: (submissionId: string, uuid: string) => void = () => {}
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
          if (Array.isArray(data.reads)) {
            for (const read of data.reads.slice(0, 128)) {
              if (
                typeof read.submissionId === 'string' &&
                read.submissionId.length <= 128 &&
                typeof read.uuid === 'string' &&
                read.uuid.length <= 128
              )
                this.onRead(read.submissionId, read.uuid)
            }
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

  healthy(): boolean {
    if (this.closed || !this.heartbeat) return false
    if (Date.now() - this.heartbeat <= 3000) return true
    this.expire()
    return false
  }

  private expire(): void {
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

  submit(text: string, fallback: () => void, submissionId?: string): boolean {
    if (!this.healthy() || this.pending.size >= 64 || Buffer.byteLength(text) > 131_072) return false
    const command: Command = { id: ++this.sequence, kind: 'submit', text, ...(submissionId ? { submissionId } : {}) }
    this.pending.set(command.id, { command, claimed: false, fallback })
    return true
  }

  arm(submissionId: string, accepted: () => void, fallback: () => void): boolean {
    if (!this.healthy() || this.pending.size >= 64) return false
    const command: Command = { id: ++this.sequence, kind: 'arm', submissionId }
    this.pending.set(command.id, { command, claimed: false, fallback, accepted })
    return true
  }

  abort(turnId: string, fallback: () => void): boolean {
    if (!this.healthy() || this.pending.size >= 64) return false
    const command: Command = { id: ++this.sequence, kind: 'abort', turnId }
    this.pending.set(command.id, { command, claimed: false, fallback })
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
    try {
      fs.rmdirSync(this.directory)
    } catch {
      /* listen failed or still closing */
    }
  }
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
    const version = await new Promise<string>((resolve) => {
      execFile(
        cli.command,
        [...cli.prefix, '--version'],
        { env: cli.env, timeout: 5000, maxBuffer: 65536 },
        (error, stdout) => {
          resolve(error ? '' : stdout.trim())
        },
      )
    })
    if (version !== `${CLI_MOD_VERSION} (Claude Code)`) return undefined
    transport = new CliModTransport(sessionId)
    await transport.listen()
    return transport
  } catch {
    transport?.dispose()
    return undefined
  }
}
