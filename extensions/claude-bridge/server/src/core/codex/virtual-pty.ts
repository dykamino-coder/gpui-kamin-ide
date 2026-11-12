// A terminal stand-in for sessions without a PTY (codex app-server threads).
//
// Session lifecycle code (destroy, kill escalation, reattach nudge, idle
// shrink) talks to `session.pty`. This object gives it the same surface:
// kill() ends the engine and reports the exit through onExit exactly once,
// write() forwards Ctrl+C as an interrupt, and the screen-size calls are kept
// as plain state. Console output is what the session prints through emit()
// (a live log of the thread); the chat stays the transcript.
import type { IDisposable, IPty } from 'node-pty'

export interface VirtualPtyHandlers {
  /** Ctrl+C from the client. */
  interrupt(): void
  /** Ends the engine; resolves with its exit code. Called at most once. */
  stop(signal?: string): Promise<number>
}

export class VirtualPty {
  readonly pid = 0
  readonly process = 'codex'
  readonly handleFlowControl = false
  cols: number
  rows: number
  private exitCode: number | undefined
  private stopping = false
  private readonly exitListeners = new Set<(e: { exitCode: number; signal?: number }) => void>()
  private readonly dataListeners = new Set<(data: string) => void>()

  constructor(
    private readonly handlers: VirtualPtyHandlers,
    cols = 120,
    rows = 40,
  ) {
    this.cols = cols
    this.rows = rows
  }

  get exited(): boolean {
    return this.exitCode !== undefined
  }

  write(data: string): void {
    if (this.exited) return
    if (data.includes('\x03')) this.handlers.interrupt()
  }

  resize(cols: number, rows: number): void {
    this.cols = cols
    this.rows = rows
  }

  onData(listener: (data: string) => void): IDisposable {
    this.dataListeners.add(listener)
    return { dispose: () => this.dataListeners.delete(listener) }
  }

  /** Console output, delivered like PTY data (Bridge Console, reattach replay). */
  emit(data: string): void {
    if (!data || this.exited) return
    for (const listener of [...this.dataListeners]) listener(data)
  }

  onExit(listener: (e: { exitCode: number; signal?: number }) => void): IDisposable {
    if (this.exitCode !== undefined) {
      const code = this.exitCode
      queueMicrotask(() => listener({ exitCode: code }))
      return { dispose: () => {} }
    }
    this.exitListeners.add(listener)
    return { dispose: () => this.exitListeners.delete(listener) }
  }

  kill(signal?: string): void {
    if (this.exited || this.stopping) return
    this.stopping = true
    void this.handlers.stop(signal).then(
      (code) => this.markExited(code),
      () => this.markExited(1),
    )
  }

  /** The engine ended on its own (app-server crash, thread failure). */
  markExited(exitCode: number): void {
    if (this.exitCode !== undefined) return
    this.exitCode = exitCode
    for (const listener of [...this.exitListeners]) listener({ exitCode })
    this.exitListeners.clear()
  }

  clear(): void {}
  pause(): void {}
  resume(): void {}

  /** Typed as node-pty's IPty for PtySession; only the members above are used. */
  asPty(): IPty {
    return this as unknown as IPty
  }
}
