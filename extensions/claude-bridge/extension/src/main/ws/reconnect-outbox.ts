import type { SubmissionUpdate } from '../../shared/types'

/** How long a message waits for its chat to reconnect before it goes back to
 *  the composer. A Bridge release restarts the server; the tab is back within
 *  seconds. Past this the user is told instead of the message landing later. */
export const RECONNECT_HOLD_MS = 60_000

export const HOLD_WARNING = 'Not delivered yet: the chat is reconnecting. It is sent as soon as the chat is back.'
export const EXPIRED_WARNING =
  'Message not delivered: the chat did not reconnect within a minute. Your text was returned to the composer.'
export const CHANGED_WARNING =
  'Message not delivered: the chat reconnected to another conversation. Your text was returned to the composer.'

export interface HeldMessage {
  id: string
  text: string
  /** The conversation the user wrote to; delivered only into that one. */
  conversationId: string
}

/**
 * Messages written while a chat with a known conversation reconnects (server
 * restart, network blip). Without it they were refused back to the composer
 * and the user had to send again once the chat was back. Each one waits for
 * the reconnect of the SAME conversation, or at most RECONNECT_HOLD_MS.
 */
export class ReconnectOutbox {
  private held = new Map<string, HeldMessage & { timer: ReturnType<typeof setTimeout> }>()

  constructor(
    private readonly emit: (update: SubmissionUpdate) => void,
    private readonly expire: (message: HeldMessage) => void,
    private readonly holdMs = RECONNECT_HOLD_MS,
  ) {}

  hold(message: HeldMessage): void {
    if (this.held.has(message.id)) return
    const timer = setTimeout(() => {
      if (this.held.delete(message.id)) this.expire(message)
    }, this.holdMs)
    this.held.set(message.id, { ...message, timer })
    this.emit({ id: message.id, text: message.text, state: 'queued', cancelable: true, warning: HOLD_WARNING })
  }

  has(id: string): boolean {
    return this.held.has(id)
  }

  /** All held messages in the order they were written; the outbox is empty after. */
  take(): HeldMessage[] {
    const all = [...this.held.values()]
    this.held.clear()
    return all.map(({ timer, ...message }) => {
      clearTimeout(timer)
      return message
    })
  }

  cancel(id: string): HeldMessage | undefined {
    const item = this.held.get(id)
    if (!item) return undefined
    clearTimeout(item.timer)
    this.held.delete(id)
    const { timer: _timer, ...message } = item
    return message
  }
}
