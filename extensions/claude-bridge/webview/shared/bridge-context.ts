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

/** The visible part of an answer that is still streaming. The closing tag of a
 *  quoted block arrives deltas later, and `withoutBridgeContext` keeps a block
 *  it cannot see the end of, so the history used to show until the stored
 *  answer replaced the stream. Here an open block hides everything after its
 *  tag, and a tail that may be the start of the tag waits for the next delta.
 *  The stream text itself is not changed: deltas keep appending to the raw text. */
export function withoutBridgeContextWhileStreaming(text: string): string {
  const start = text.indexOf(OPEN)
  if (start >= 0) {
    const end = text.lastIndexOf(CLOSE)
    if (end < start) return text.slice(0, start).trimEnd()
    return withoutBridgeContextWhileStreaming(withoutBridgeContext(text))
  }
  return text.slice(0, text.length - openTagPrefixAtEnd(text))
}

/** Length of the longest proper prefix of the opening tag that ends `text`. */
function openTagPrefixAtEnd(text: string): number {
  for (let n = Math.min(OPEN.length - 1, text.length); n > 0; n--) {
    if (text.endsWith(OPEN.slice(0, n))) return n
  }
  return 0
}

export function hasBridgeContext(text: string): boolean {
  return text.includes(OPEN)
}
