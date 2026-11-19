// Hidden context Bridge adds after a user's message (server engine/handoff.ts):
// the conversation so far, for an engine that joins the chat. The model reads
// it; the chat shows only what the user wrote.
import type { JsonlEntryData } from '../types/jsonl'

const OPEN = '<bridge-context>'
const CLOSE = '</bridge-context>'

/** The block runs from its first opening tag to the LAST closing one: the
 *  history inside can itself quote an earlier block (an answer that repeated
 *  its prompt), and stopping at the first closing tag left the rest of the
 *  history in the chat and the queue. */
export function withoutBridgeContext(text: string): string {
  const start = text.indexOf(OPEN)
  if (start < 0) return text
  const end = text.lastIndexOf(CLOSE)
  if (end < start) return text
  return `${text.slice(0, start)}\n${text.slice(end + CLOSE.length)}`.trim()
}

/** Remove the hidden block from a user row, in place (rows come fresh from IPC). */
export function stripBridgeContext(entry: JsonlEntryData): void {
  if (entry.type !== 'user') return
  const message = entry.message as { content?: unknown } | undefined
  if (!message) return
  const content = message.content
  if (typeof content === 'string') {
    if (content.includes(OPEN)) message.content = withoutBridgeContext(content)
    return
  }
  if (!Array.isArray(content)) return
  for (const block of content as Array<{ type?: string; text?: unknown }>) {
    if (block?.type === 'text' && typeof block.text === 'string' && block.text.includes(OPEN))
      block.text = withoutBridgeContext(block.text)
  }
}
