// Client IDs follow native queue deliveries; the stored prompt row in the transcript is the read receipt.
import type { PtySession } from './types'
import type { CliModTransport } from './cli-mod-transport'
import { getSessionInputSnapshot, isTerminalResponseInput, submitCoordinatedText } from './session-input-coordinator'
import { sendToClient } from './session-io'
import { normalizePrompt, promptRow, storesPrompt } from './transcript-prompts'
import type { JsonlEntry } from '../../shared/jsonl-types'

type State = 'queued' | 'sending' | 'native' | 'read' | 'cancelled' | 'uncertain' | 'fallback'
interface Submission {
  id: string
  /** What the CLI gets and stores in its prompt row (may carry a context block). */
  text: string
  /** What the user wrote: the receipt text, the client's submission identity. */
  userText: string
  state: State
  uuid?: string
  warning?: string
  interrupt?: boolean
  /** Server time of acceptance: lets a client settle a lost receipt from the transcript. */
  createdAt: number
  /** Server time the mod reported the CLI accepted an idle submit. */
  acceptedAt?: number
}

/** A submission an engine switch takes from the leaving CLI. */
export interface CarriedSubmission {
  id: string
  /** What the user wrote (the joining engine adds its own context). */
  text: string
  createdAt: number
}

/** A text handed to the CLI whose stored prompt row is not seen yet. */
interface Awaiting {
  item: Submission
  normalized: string
  /** Server time the text went to the CLI; an older row cannot store it. */
  since: number
}

/** Rows written up to this long before hand-off still count: the CLI and the
 * server round the same instant independently (the clients use the same margin). */
const CLOCK_MARGIN_MS = 2000
/** An accepted idle submit whose prompt row has not appeared this long after
 * the CLI went idle again is reported unconfirmed instead of blocking the queue. */
const UNMATCHED_MS = 5000

export class CliSubmissions {
  capable = true
  private items = new Map<string, Submission>()
  private dispatching: string | undefined
  private timer: ReturnType<typeof setInterval>
  /** In hand-off order, which is the order the CLI stores them in. */
  private awaiting: Awaiting[] = []
  /** Rows already matched: a range the watcher re-reads must not match twice. */
  private matchedRows = new Set<string>()
  /** The chat moved to another engine (retire). */
  private retired = false

  constructor(
    private session: PtySession,
    private mod: CliModTransport,
    private busy: () => boolean,
    private abort: () => boolean,
  ) {
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
    if (this.retired) return
    sendToClient(this.session.ws, {
      type: 'session:submission',
      sessionId: this.session.id,
      id: item.id,
      text: item.userText,
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

  /** The text is now the CLI's: its next stored prompt row with that text is its receipt. */
  private handedOff(item: Submission): void {
    if (this.awaiting.length >= 256) this.awaiting.shift()
    this.awaiting.push({ item, normalized: normalizePrompt(item.text), since: Date.now() })
  }

  /** Newly appended transcript records. Each prompt row is the receipt of the
   * earliest handed-off text it stores; rows typed in the console store none
   * and are skipped. Console edits of the native queue disable receipts
   * (rawInput) before they could reorder rows. */
  transcript(entries: readonly JsonlEntry[]): void {
    if (!this.awaiting.length) return
    for (const entry of entries) {
      const row = promptRow(entry)
      if (!row || this.matchedRows.has(row.uuid)) continue
      const index = this.awaiting.findIndex(
        (wait) => !(row.at < wait.since - CLOCK_MARGIN_MS) && storesPrompt(row, wait.normalized),
      )
      if (index < 0) continue
      const [{ item }] = this.awaiting.splice(index, 1) as [Awaiting]
      if (this.matchedRows.size >= 512) this.matchedRows.clear()
      this.matchedRows.add(row.uuid)
      this.read(item, row.uuid)
      if (!this.awaiting.length) return
    }
  }

  private read(item: Submission, uuid: string): void {
    if (!this.capable || item.state === 'cancelled' || this.items.get(item.id) !== item) return
    item.state = 'read'
    item.uuid = uuid
    if (this.dispatching === item.id) this.dispatching = undefined
    this.emit(item)
  }

  /** A submission the CLI has not read yet: the session still owes it a turn. */
  pending(): boolean {
    for (const item of this.items.values()) if (item.state === 'queued' || item.state === 'sending') return true
    return false
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

  /** Engine switch: this CLI goes away with whatever it holds. Messages it
   * never got move to the joining engine (in order); messages it got but did
   * not confirm go back to the user. From here on this queue reports nothing:
   * the joining session owns these ids, and a late ack of this CLI (its mod
   * accepting an armed submit, its exit marking items uncertain) would
   * contradict what the joining one reports. */
  retire(): { moved: CarriedSubmission[]; returned: CarriedSubmission[] } {
    const moved = new Set(this.handoff())
    const out = { moved: [] as CarriedSubmission[], returned: [] as CarriedSubmission[] }
    for (const item of this.items.values()) {
      const carried = { id: item.id, text: item.userText, createdAt: item.createdAt }
      if (moved.has(item.id)) out.moved.push(carried)
      else if (item.state === 'native' || item.state === 'sending') out.returned.push(carried)
    }
    this.retired = true
    this.items.clear()
    this.awaiting = []
    this.matchedRows.clear()
    return out
  }

  submit(id: string, text: string, userText = text): boolean {
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
    const item: Submission = { id, text, userText, state: 'queued', createdAt: Date.now() }
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
    const sent = this.dispatching === undefined ? undefined : this.items.get(this.dispatching)
    if (
      sent?.state === 'sending' &&
      sent.acceptedAt !== undefined &&
      Date.now() - sent.acceptedAt > UNMATCHED_MS &&
      !this.busy()
    ) {
      sent.state = 'uncertain'
      sent.warning = 'Delivery could not be confirmed. Check the transcript before resending.'
      this.dispatching = undefined
      this.emit(sent)
    }
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
      this.handedOff(item)
      const accepted = () => {
        item.acceptedAt = Date.now()
      }
      if (!this.mod.submit(item.text, () => this.fallback(item), item.id, accepted)) this.fallback(item)
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
          this.handedOff(item)
          submitCoordinatedText(this.session, item.text)
          this.dispatching = undefined
        },
        () => this.fallback(item),
      )
    )
      this.fallback(item)
  }
}
