// Client IDs follow native queue deliveries without changing the transcript watcher.
import type { PtySession } from './types'
import type { CliModTransport } from './cli-mod-transport'
import { getSessionInputSnapshot, isTerminalResponseInput, submitCoordinatedText } from './session-input-coordinator'
import { sendToClient } from './session-io'

type State = 'queued' | 'sending' | 'native' | 'read' | 'cancelled' | 'uncertain' | 'fallback'
interface Submission {
  id: string
  text: string
  state: State
  uuid?: string
  warning?: string
  interrupt?: boolean
}

export class CliSubmissions {
  capable = true
  private items = new Map<string, Submission>()
  private dispatching: string | undefined
  private timer: ReturnType<typeof setInterval>

  constructor(
    private session: PtySession,
    private mod: CliModTransport,
    private busy: () => boolean,
    private abort: () => boolean,
  ) {
    mod.onRead = (id, uuid) => {
      const item = this.items.get(id)
      if (!this.capable || !item || item.state === 'cancelled') return
      item.state = 'read'
      item.uuid = uuid
      if (this.dispatching === id) this.dispatching = undefined
      this.emit(item)
    }
    this.timer = setInterval(() => {
      if (session.state !== 'running') {
        clearInterval(this.timer)
        return
      }
      this.drain()
    }, 100)
    this.timer.unref()
  }

  private emit(item: Submission): void {
    sendToClient(this.session.ws, {
      type: 'session:submission',
      sessionId: this.session.id,
      id: item.id,
      text: item.text,
      state: item.state,
      uuid: item.uuid,
      cancelable: item.state === 'queued',
      warning: item.warning,
    })
  }

  replay(): void {
    for (const item of this.items.values()) this.emit(item)
  }

  submit(id: string, text: string): boolean {
    if (!id || id.length > 128 || Buffer.byteLength(text) > 131_072) return false
    const existing = this.items.get(id)
    if (existing) {
      this.emit(existing)
      return true
    }
    if (
      !this.capable ||
      !this.mod.healthy() ||
      [...this.items.values()].filter((i) => i.state === 'queued').length >= 64
    )
      return false
    // Retain a bounded receipt window for reconnect/repeated frames.
    if (this.items.size >= 256) {
      for (const [key, item] of this.items) {
        if (['read', 'cancelled', 'fallback'].includes(item.state)) {
          this.items.delete(key)
          break
        }
      }
      if (this.items.size >= 256) return false
    }
    const item: Submission = { id, text, state: 'queued' }
    this.items.set(id, item)
    this.emit(item)
    this.drain()
    return true
  }

  action(id: string, action: string): void {
    const item = this.items.get(id)
    if (!item) return
    if (action === 'cancel' && item.state === 'queued') {
      item.state = 'cancelled'
      this.emit(item)
    } else if (action === 'push') this.drain()
    else if (action === 'interrupt' && ['queued', 'native'].includes(item.state)) {
      if (this.abort() && item.state === 'queued') item.interrupt = true
      // Native CLI owns an already-handed-off message across abort. Resubmitting duplicates it.
    }
  }

  disableReceipts(): void {
    if (!this.capable) return
    this.capable = false
    this.uncertain()
    sendToClient(this.session.ws, {
      type: 'session:submission',
      sessionId: this.session.id,
      id: '',
      capable: false,
      warning: 'Native queue edited in the console; delivery receipts are disabled until a new CLI session.',
    })
  }

  rawInput(data: string): void {
    if (isTerminalResponseInput(data)) return
    if (/\x1b(?:\[A|OA)/.test(data) || this.dispatching) this.disableReceipts()
  }

  uncertain(): void {
    for (const item of this.items.values()) {
      if (item.state !== 'native' && item.id !== this.dispatching) continue
      item.state = 'uncertain'
      item.warning = 'Delivery could not be confirmed. Check the transcript before resending.'
      this.emit(item)
    }
    this.dispatching = undefined
  }

  private fallback(item: Submission): void {
    item.state = 'fallback'
    item.warning = 'Bridge mod unavailable; using native terminal delivery.'
    this.emit(item)
    this.dispatching = undefined
    submitCoordinatedText(this.session, item.text)
  }

  private drain(): void {
    if (this.dispatching) return
    const item = [...this.items.values()].find((i) => i.state === 'queued')
    if (!item) return
    const input = getSessionInputSnapshot(this.session)
    if (input.rawInputDirty) {
      if (!item.warning) {
        item.warning = 'The terminal composer has a draft. Clear or send it before delivering this message.'
        this.emit(item)
      }
      return
    }
    if (input.activeKind || input.queuedSubmissions || input.pendingMaintenance.length) return
    if (item.interrupt && this.busy()) return
    if (!this.capable || !this.mod.healthy()) {
      this.fallback(item)
      return
    }
    this.dispatching = item.id
    item.state = 'sending'
    item.warning = undefined
    this.emit(item)
    if (!this.busy()) {
      if (!this.mod.submit(item.text, () => this.fallback(item), item.id)) this.fallback(item)
    } else if (
      !this.mod.arm(
        item.id,
        () => {
          if (!this.capable || item.state !== 'sending') {
            this.mod.arm(
              '',
              () => {},
              () => {},
            )
            return
          }
          // Recheck after the handshake: console input may have arrived while it was in flight.
          const now = getSessionInputSnapshot(this.session)
          if (now.rawInputDirty || now.activeKind || now.queuedSubmissions) {
            this.mod.arm(
              '',
              () => {},
              () => {},
            )
            item.state = 'uncertain'
            item.warning = 'Terminal input changed during delivery. Check the console before resending.'
            this.dispatching = undefined
            this.emit(item)
            return
          }
          item.state = 'native'
          this.emit(item)
          submitCoordinatedText(this.session, item.text)
          this.dispatching = undefined
        },
        () => this.fallback(item),
      )
    )
      this.fallback(item)
  }
}
