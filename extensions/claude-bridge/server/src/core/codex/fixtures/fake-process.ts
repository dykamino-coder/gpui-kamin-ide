// In-memory stand-in for a codex app-server process: Bridge's real
// JsonRpcConnection on one side, a scriptable peer on the other.
import { PassThrough } from 'node:stream'
import type { CodexExit } from '../app-server'
import type { CodexProcess } from '../codex-host'
import { JsonRpcConnection } from '../jsonrpc'

type Handler = (params: any) => unknown

export class FakeCodexProcess implements CodexProcess {
  readonly rpc: JsonRpcConnection
  readonly received: Array<{ id?: unknown; method?: string; params?: any; result?: unknown; error?: unknown }> = []
  exited: CodexExit | undefined
  private readonly toBridge = new PassThrough()
  private readonly fromBridge = new PassThrough()
  private readonly exitListeners = new Set<(exit: CodexExit) => void>()
  private partial = ''
  closeCalls = 0

  constructor(private readonly handlers: Record<string, Handler> = {}) {
    this.rpc = new JsonRpcConnection(this.toBridge, this.fromBridge)
    this.fromBridge.on('data', (chunk: Buffer) => {
      this.partial += chunk.toString('utf8')
      let i: number
      while ((i = this.partial.indexOf('\n')) >= 0) {
        const message = JSON.parse(this.partial.slice(0, i))
        this.partial = this.partial.slice(i + 1)
        this.received.push(message)
        if (message.method && message.id !== undefined) this.answer(message)
      }
    })
  }

  private answer(message: { id: unknown; method: string; params: unknown }): void {
    const handler = this.handlers[message.method]
    if (!handler) {
      this.send({ id: message.id, error: { code: -32601, message: `fake: ${message.method}` } })
      return
    }
    try {
      this.send({ id: message.id, result: handler(message.params) ?? {} })
    } catch (error) {
      this.send({ id: message.id, error: { code: -32000, message: (error as Error).message } })
    }
  }

  /** Writes raw frames as codex would; several frames in one write share a chunk. */
  send(...frames: unknown[]): void {
    this.toBridge.write(frames.map((f) => JSON.stringify(f) + '\n').join(''))
  }

  notify(method: string, params: unknown, emittedAtMs?: number): void {
    this.send(emittedAtMs === undefined ? { method, params } : { method, params, emittedAtMs })
  }

  requests(method: string): any[] {
    return this.received.filter((m) => m.method === method && m.id !== undefined).map((m) => m.params)
  }

  /** Replies Bridge sent to server requests. */
  replies(): Array<{ id: unknown; result?: unknown; error?: unknown }> {
    return this.received.filter((m) => m.method === undefined) as any
  }

  onExit(listener: (exit: CodexExit) => void): () => void {
    if (this.exited) {
      listener(this.exited)
      return () => {}
    }
    this.exitListeners.add(listener)
    return () => this.exitListeners.delete(listener)
  }

  crash(exit: CodexExit = { code: 1, signal: null }): void {
    if (this.exited) return
    this.exited = exit
    this.rpc.close()
    for (const l of this.exitListeners) l(exit)
  }

  async close(): Promise<CodexExit> {
    this.closeCalls++
    this.crash({ code: 0, signal: null })
    return this.exited!
  }
}

export const flush = () => new Promise((r) => setImmediate(r))
