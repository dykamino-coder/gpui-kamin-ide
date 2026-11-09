import headless from '@xterm/headless'
import serialization from '@xterm/addon-serialize'

const { Terminal } = headless
const { SerializeAddon } = serialization
export const SCROLLBACK = 5000
const MAX_PENDING = 2 * 1024 * 1024
const MAX_SNAPSHOT = 8 * 1024 * 1024

/** Ordered parser + snapshots; never retain an unbounded raw output log. */
export class TerminalScreen {
  private readonly terminal: InstanceType<typeof Terminal>
  private readonly serializer = new SerializeAddon()
  private queue = Promise.resolve()
  private pending = 0
  private closed = false
  private overflowed = false

  constructor(cols: number, rows: number, private overflow: () => void, respond: (data: string) => void = () => {}) {
    this.terminal = new Terminal({ cols, rows, scrollback: SCROLLBACK, allowProposedApi: true })
    this.terminal.loadAddon(this.serializer)
    this.terminal.onData(respond)
  }

  write(data: string, committed: () => void): void {
    if (this.closed || this.overflowed) return
    const bytes = Buffer.byteLength(data)
    if (this.pending + bytes > MAX_PENDING) {
      this.overflowed = true
      this.overflow()
      return
    }
    this.pending += bytes
    void this.order(async () => {
      await new Promise<void>(resolve => this.terminal.write(data, resolve))
      this.pending -= bytes
      if (!this.closed) committed()
    }).catch(() => { if (!this.closed) this.overflow() })
  }

  order(run: () => void | Promise<void>): Promise<void> {
    const operation = this.queue.then(async () => { if (!this.closed) await run() })
    this.queue = operation.catch(() => {})
    return operation
  }

  resize(cols: number, rows: number): void { this.terminal.resize(cols, rows) }

  snapshot(): { screen: string; truncated: boolean } {
    let lines = SCROLLBACK
    let screen = this.serializer.serialize({ scrollback: lines })
    // The websocket limit applies to JSON, where an ESC becomes six bytes.
    while (Buffer.byteLength(JSON.stringify(screen)) > MAX_SNAPSHOT && lines > 0) {
      lines = Math.floor(lines / 2)
      screen = this.serializer.serialize({ scrollback: lines })
    }
    if (Buffer.byteLength(JSON.stringify(screen)) > MAX_SNAPSHOT) throw new Error('Terminal screen exceeds snapshot limit')
    return { screen, truncated: lines < SCROLLBACK }
  }

  dispose(): void {
    if (this.closed) return
    this.closed = true
    this.terminal.dispose()
  }
}
