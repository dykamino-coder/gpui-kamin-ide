// Hidden context Bridge adds after a user's message (server engine/handoff.ts):
// the conversation so far, for an engine that joins the chat. The model reads
// it; the chat shows only what the user wrote. Shared by the webview and the
// browser workspace host (queue and receipt matching against raw rows).

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

export function hasBridgeContext(text: string): boolean {
  return text.includes(OPEN)
}
