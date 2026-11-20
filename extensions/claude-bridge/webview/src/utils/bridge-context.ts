// The chat shows what people wrote, never Bridge's hidden history block
// (shared/bridge-context.ts): not in the user's row, and not in an answer that
// quotes the block verbatim (a model asked to repeat its prompt does).
import type { JsonlEntryData } from '../types/jsonl'
import { hasBridgeContext, withoutBridgeContext } from '../../shared/bridge-context'

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
