// ============================================================================
// Newline-delimited JSON-RPC over a pair of byte streams — the framing
// `codex app-server` uses on stdio. Codex omits the `"jsonrpc": "2.0"` member,
// so neither side sends it; incoming messages are classified by shape:
// `{id, method}` server request, `{method}` notification, `{id, result|error}`
// response. Every request has a deadline and every pending request is settled
// when the connection closes, so a dead child can never strand a caller.
// ============================================================================

import { StringDecoder } from 'node:string_decoder'
import type { Readable, Writable } from 'node:stream'
import type { RequestId } from './protocol'

export class JsonRpcError extends Error {
  constructor(
    readonly code: number,
    message: string,
    readonly data?: unknown,
  ) {
    super(message)
    this.name = 'JsonRpcError'
  }
}

export class JsonRpcClosedError extends Error {
  constructor(reason?: string) {
    super(reason ? `JSON-RPC connection closed: ${reason}` : 'JSON-RPC connection closed')
    this.name = 'JsonRpcClosedError'
  }
}

export class JsonRpcTimeoutError extends Error {
  constructor(
    readonly method: string,
    readonly timeoutMs: number,
  ) {
    super(`JSON-RPC request ${method} timed out after ${timeoutMs}ms`)
    this.name = 'JsonRpcTimeoutError'
  }
}

export const METHOD_NOT_FOUND = -32601
export const INTERNAL_ERROR = -32603

/** `message` is the whole frame: codex puts `emittedAtMs` beside `params`. */
export type NotificationListener = (method: string, params: unknown, message: Record<string, unknown>) => void
/** Returns the result, or throws a JsonRpcError to answer with an error. */
export type ServerRequestHandler = (method: string, params: unknown, id: RequestId) => unknown | Promise<unknown>

export interface JsonRpcConnectionOptions {
  /** Deadline for requests that do not pass their own. */
  defaultTimeoutMs?: number
  /** A line longer than this closes the connection: the peer is not speaking the protocol. */
  maxLineBytes?: number
  /** Malformed or unexpected input that did not close the connection. */
  onProtocolError?: (error: Error) => void
  /** Sees every parsed message in both directions (fixture recording, diagnostics). */
  onTrace?: (direction: 'in' | 'out', message: Record<string, unknown>) => void
}

export interface RequestOptions {
  timeoutMs?: number
  signal?: AbortSignal
}

interface Pending {
  method: string
  resolve: (value: unknown) => void
  reject: (error: Error) => void
  timer: NodeJS.Timeout
  cleanup: () => void
}

export class JsonRpcConnection {
  private nextId = 1
  private readonly pending = new Map<RequestId, Pending>()
  private readonly listeners = new Set<NotificationListener>()
  private readonly closeListeners = new Set<(error: Error) => void>()
  private handler: ServerRequestHandler | undefined
  private buffer = ''
  private readonly decoder = new StringDecoder('utf8')
  private closedWith: Error | undefined
  private readonly defaultTimeoutMs: number
  private readonly maxLineBytes: number

  constructor(
    private readonly readable: Readable,
    private readonly writable: Writable,
    private readonly options: JsonRpcConnectionOptions = {},
  ) {
    this.defaultTimeoutMs = options.defaultTimeoutMs ?? 30_000
    this.maxLineBytes = options.maxLineBytes ?? 64 * 1024 * 1024
    readable.on('data', this.onData)
    readable.on('end', this.onEnd)
    readable.on('error', this.onStreamError)
    writable.on('error', this.onStreamError)
  }

  get closed(): boolean {
    return this.closedWith !== undefined
  }

  request<T = unknown>(method: string, params?: unknown, options: RequestOptions = {}): Promise<T> {
    if (this.closedWith) return Promise.reject(this.closedWith)
    if (options.signal?.aborted) return Promise.reject(options.signal.reason ?? new Error('aborted'))
    const id = this.nextId++
    const timeoutMs = options.timeoutMs ?? this.defaultTimeoutMs
    return new Promise<T>((resolve, reject) => {
      const onAbort = () => settle(() => reject(options.signal?.reason ?? new Error('aborted')))
      const settle = (fn: () => void) => {
        const entry = this.pending.get(id)
        if (!entry) return
        this.pending.delete(id)
        entry.cleanup()
        fn()
      }
      const timer = setTimeout(() => settle(() => reject(new JsonRpcTimeoutError(method, timeoutMs))), timeoutMs)
      timer.unref?.()
      options.signal?.addEventListener('abort', onAbort, { once: true })
      this.pending.set(id, {
        method,
        resolve: resolve as (value: unknown) => void,
        reject,
        timer,
        cleanup: () => {
          clearTimeout(timer)
          options.signal?.removeEventListener('abort', onAbort)
        },
      })
      try {
        this.write(params === undefined ? { id, method } : { id, method, params })
      } catch (error) {
        settle(() => reject(error as Error))
      }
    })
  }

  notify(method: string, params?: unknown): void {
    if (this.closedWith) throw this.closedWith
    this.write(params === undefined ? { method } : { method, params })
  }

  onNotification(listener: NotificationListener): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  onClose(listener: (error: Error) => void): () => void {
    if (this.closedWith) {
      listener(this.closedWith)
      return () => {}
    }
    this.closeListeners.add(listener)
    return () => this.closeListeners.delete(listener)
  }

  setServerRequestHandler(handler: ServerRequestHandler | undefined): void {
    this.handler = handler
  }

  /** Idempotent. Rejects every pending request with the close reason. */
  close(reason: Error = new JsonRpcClosedError()): void {
    if (this.closedWith) return
    this.closedWith = reason
    this.readable.off('data', this.onData)
    this.readable.off('end', this.onEnd)
    for (const [id, entry] of this.pending) {
      this.pending.delete(id)
      entry.cleanup()
      entry.reject(reason)
    }
    for (const listener of this.closeListeners) {
      try {
        listener(reason)
      } catch {
        /* a listener must not keep others from observing the close */
      }
    }
    this.closeListeners.clear()
    this.listeners.clear()
  }

  private write(message: Record<string, unknown>): void {
    if (this.closedWith) throw this.closedWith
    this.options.onTrace?.('out', message)
    this.writable.write(JSON.stringify(message) + '\n')
  }

  private readonly onData = (chunk: Buffer | string) => {
    this.buffer += typeof chunk === 'string' ? chunk : this.decoder.write(chunk)
    let newline: number
    while ((newline = this.buffer.indexOf('\n')) >= 0) {
      const line = this.buffer.slice(0, newline).trim()
      this.buffer = this.buffer.slice(newline + 1)
      if (line) this.dispatch(line)
      if (this.closedWith) return
    }
    // Character count never exceeds the UTF-8 byte count, so a line within the
    // byte limit is never cut.
    if (this.buffer.length > this.maxLineBytes) {
      this.buffer = ''
      this.close(new JsonRpcClosedError(`line exceeds ${this.maxLineBytes} bytes`))
    }
  }

  private readonly onEnd = () => this.close(new JsonRpcClosedError('peer closed its output'))
  private readonly onStreamError = (error: Error) => this.close(new JsonRpcClosedError(error.message))

  private protocolError(message: string): void {
    this.options.onProtocolError?.(new Error(message))
  }

  private dispatch(line: string): void {
    let message: Record<string, unknown>
    try {
      const parsed: unknown = JSON.parse(line)
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) throw new Error('not an object')
      message = parsed as Record<string, unknown>
    } catch {
      this.protocolError(`unparseable line (${line.length} chars)`)
      return
    }
    this.options.onTrace?.('in', message)
    const id = message.id as RequestId | undefined
    const method = typeof message.method === 'string' ? message.method : undefined
    if (method !== undefined && id !== undefined) {
      void this.answer(id, method, message.params)
      return
    }
    if (method !== undefined) {
      for (const listener of this.listeners) {
        try {
          listener(method, message.params, message)
        } catch (error) {
          this.protocolError(`notification listener failed for ${method}: ${(error as Error).message}`)
        }
      }
      return
    }
    if (id === undefined) {
      this.protocolError('message without id or method')
      return
    }
    const entry = this.pending.get(id)
    if (!entry) {
      this.protocolError(`response for unknown request id ${String(id)}`)
      return
    }
    this.pending.delete(id)
    entry.cleanup()
    const error = message.error as { code?: unknown; message?: unknown; data?: unknown } | undefined
    if (error) {
      entry.reject(
        new JsonRpcError(
          typeof error.code === 'number' ? error.code : INTERNAL_ERROR,
          typeof error.message === 'string' ? error.message : `${entry.method} failed`,
          error.data,
        ),
      )
    } else {
      entry.resolve(message.result)
    }
  }

  private async answer(id: RequestId, method: string, params: unknown): Promise<void> {
    let reply: Record<string, unknown>
    try {
      if (!this.handler) throw new JsonRpcError(METHOD_NOT_FOUND, `${method} is not supported by Bridge`)
      reply = { id, result: (await this.handler(method, params, id)) ?? {} }
    } catch (error) {
      const rpc =
        error instanceof JsonRpcError ? error : new JsonRpcError(INTERNAL_ERROR, (error as Error)?.message ?? 'failed')
      reply = {
        id,
        error: { code: rpc.code, message: rpc.message, ...(rpc.data === undefined ? {} : { data: rpc.data }) },
      }
    }
    if (this.closedWith) return
    try {
      this.write(reply)
    } catch {
      /* closed while the handler ran; nothing left to answer */
    }
  }
}
