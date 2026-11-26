// The chat shows what people wrote, never Bridge's hidden history block
// (shared/bridge-context.ts): not in the user's row, and not in an answer that
// quotes the block verbatim (a model asked to repeat its prompt does).
import type { JsonlEntryData } from '../types/jsonl'
import { hasBridgeContext, withoutBridgeContext, withoutBridgeContextWhileStreaming } from '../../shared/bridge-context'

export { withoutBridgeContext }

/** Remove the hidden block from a user or assistant row, in place (rows come fresh from IPC). */
export function stripBridgeContext(entry: JsonlEntryData): void {
  if (entry.type !== 'user' && entry.type !== 'assistant') return
  const message = entry.message as { content?: unknown } | undefined
  if (!message) return
  const content = message.content
  if (typeof content === 'string') {
    if (hasBridgeContext(content)) message.content = withoutBridgeContext(content)
    return
  }
  if (!Array.isArray(content)) return
  for (const block of content as Array<{ type?: string; text?: unknown }>) {
    if (block?.type === 'text' && typeof block.text === 'string' && hasBridgeContext(block.text))
      block.text = withoutBridgeContext(block.text)
  }
}

/** What a streaming answer shows: its text blocks without a hidden block that
 *  is still arriving (shared/bridge-context.ts). Returns the same entry when
 *  nothing changes, so a stream without the block costs no copy per frame.
 *  The stub itself stays raw because deltas keep appending to it. */
export function streamingView(entry: JsonlEntryData): JsonlEntryData {
  const message = entry.message as { content?: unknown } | undefined
  const content = message?.content
  if (typeof content === 'string') {
    const shown = withoutBridgeContextWhileStreaming(content)
    return shown === content ? entry : { ...entry, message: { ...entry.message!, content: shown } }
  }
  if (!Array.isArray(content)) return entry
  let next: unknown[] | null = null
  content.forEach((block: { type?: string; text?: unknown }, i: number) => {
    if (block?.type !== 'text' || typeof block.text !== 'string') return
    const shown = withoutBridgeContextWhileStreaming(block.text)
    if (shown === block.text) return
    next ??= content.slice()
    next[i] = { ...block, text: shown }
  })
  return next ? { ...entry, message: { ...entry.message!, content: next } } as JsonlEntryData : entry
}