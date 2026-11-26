// Completion receipts for slash commands the terminal delivers on purpose.
// `/resume`, `/clear` and `/compact` leave no row in the transcript the client
// watches (a resume writes nothing at all, a clear and a compact start another
// file), so a queue item for them can never be matched by text. The CLI's
// SessionStart hook is the correlated signal: its source names what ran and
// its session_id names the conversation it now runs.
import type { PtySession } from './types'
import { sendToClient } from './session-io'

type SessionStartSource = 'resume' | 'clear' | 'compact'

interface PendingCommand {
  id: string
  text: string
  source: SessionStartSource
  /** Conversation the command asked for; undefined accepts any (picker, search). */
  target?: string
  createdAt: number
}

const COMMANDS: Record<string, SessionStartSource> = {
  resume: 'resume',
  continue: 'resume',
  clear: 'clear',
  reset: 'clear',
  new: 'clear',
  compact: 'compact',
}
const CONVERSATION_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const MAX_PENDING = 32
const MAX_SETTLED = 32

const pending = new WeakMap<PtySession, PendingCommand[]>()
const settled = new WeakMap<PtySession, Record<string, unknown>[]>()

/** Which SessionStart source completes this command text, if any. */
export function sessionStartCommand(text: string): { source: SessionStartSource; target?: string } | undefined {
  const match = /^\s*\/([A-Za-z][\w-]*)(?:\s+([\s\S]*))?$/.exec(text)
  const source = match && COMMANDS[match[1]!.toLowerCase()]
  if (!source) return undefined
  // The CLI reads the command line only; a context block may follow it.
  const arg = match[2]?.split('\n')[0]?.trim() ?? ''
  // A resume by title or search term cannot be checked against the id.
  return source === 'resume' && CONVERSATION_ID.test(arg) ? { source, target: arg.toLowerCase() } : { source }
}

/** Remember a terminal-delivered command whose completion SessionStart reports. */
export function trackTerminalCommand(session: PtySession, id: string, text: string, createdAt: number): void {
  const command = sessionStartCommand(text)
  if (!command) return
  const list = pending.get(session) ?? []
  if (list.some((item) => item.id === id)) return
  list.push({ id, text, createdAt, ...command })
  if (list.length > MAX_PENDING) list.shift()
  pending.set(session, list)
}

/** SessionStart from this session's CLI: settle the oldest command it completes. */
export function settleTerminalCommand(session: PtySession, source: unknown, conversationId: unknown): void {
  const list = pending.get(session)
  if (!list?.length || typeof source !== 'string') return
  const id = typeof conversationId === 'string' ? conversationId.toLowerCase() : ''
  const index = list.findIndex((item) => item.source === source && (item.target === undefined || item.target === id))
  if (index < 0) return
  const [item] = list.splice(index, 1)
  const receipt = {
    type: 'session:submission',
    sessionId: session.id,
    id: item!.id,
    text: item!.text,
    state: 'fallback',
    cancelable: false,
    committed: true,
    createdAt: item!.createdAt,
  }
  const done = settled.get(session) ?? []
  done.push(receipt)
  if (done.length > MAX_SETTLED) done.shift()
  settled.set(session, done)
  sendToClient(session.ws, receipt)
}

/** Reattach: a client that missed a completion still learns it. */
export function replayTerminalCommandReceipts(session: PtySession): void {
  for (const receipt of settled.get(session) ?? []) sendToClient(session.ws, { ...receipt, sessionId: session.id })
}
