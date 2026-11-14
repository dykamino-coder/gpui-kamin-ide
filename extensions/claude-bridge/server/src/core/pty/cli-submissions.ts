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
  /** Server time of acceptance: lets a client settle a lost receipt from the transcript. */
  createdAt: number
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
      createdAt: item.createdAt,
    })
  }

  replay(): void {
    for (const item of this.items.values()) this.emit(item)
  }

  /** Every id still tracked; a client drops pending receipts missing from it. */
  ids(): string[] {
    return [...this.items.keys()]
  }

  /** Freeze before PTY destruction. Only these IDs can move automatically to
   * another CLI; native/claimed submits require the user's delivery decision. */
  handoff(): string[] {
    clearInterval(this.timer)
    this.capable = false
    const safe: string[] = []
    for (const item of this.items.values()) {
      if (item.state === 'queued' || (item.state === 'sending' && this.mod.withdrawSubmission(item.id))) {
        safe.push(item.id)
        item.state = 'cancelled'
      }
    }
    this.dispatching = undefined
    return safe
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
    const item: Submission = { id, text, state: 'queued', createdAt: Date.now() }
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
    const item = [...this.items.values()].find((i) => i.state === 'queued')
    if (!item) return
    const wait = (warning: string) => {
      if (item.warning === warning) return
      item.warning = warning
      this.emit(item)
    }
    if (this.dispatching) {
      wait('Waiting for the CLI to confirm an earlier message. Check its delivery status in the chat.')
      return
    }
    const input = getSessionInputSnapshot(this.session)
    if (input.rawInputDirty) {
      wait('The terminal composer has a draft. Clear or send it before delivering this message.')
      return
    }
    // Pending maintenance waits for idle; user input must not wait behind it
    // during a long turn. The coordinator still serializes active transactions.
    if (input.activeKind || input.queuedSubmissions) {
      wait('Waiting for the current terminal input transaction to finish.')
      return
    }
    if (item.interrupt && this.busy()) {
      wait('Waiting for the CLI to finish interrupting the current turn.')
      return
    }
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
