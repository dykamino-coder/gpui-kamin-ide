import type { ContentBlock, JsonlEntryData } from '../../types/jsonl'

/** The row's content, its thinking blocks carrying the row's
 *  `thinkingDurationMs`: only the thinking row has it, so a bubble merged from
 *  several rows would otherwise lose it to the last row's fields. Copies the
 *  blocks it changes, never mutates the store. */
export function contentWithThinkingDuration(entry: JsonlEntryData): ContentBlock[] {
  const content = Array.isArray(entry.message?.content) ? entry.message.content as ContentBlock[] : []
  const ms = entry.thinkingDurationMs
  if (typeof ms !== 'number' || !content.some(block => block?.type === 'thinking' && block.durationMs === undefined)) return content
  return content.map(block => block?.type === 'thinking' && block.durationMs === undefined ? { ...block, durationMs: ms } : block)
}
