// Whether a row's renderer draws anything (INC-2026-0071 / BR-22).
//
// Several entry components decide to render nothing only once they are mounted
// (AssistantEntry with no visible block, SystemEntry for bookkeeping subtypes or
// empty content, CompactSummaryEntry without text, an empty tool group).
// JsonlEntry used to return their vnode anyway, so the viewer mounted an empty
// wrapper for the row and counted it as drawn. These checks are the single
// source for both: the components bail out with them, and JsonlEntry returns
// null up front, which is what the drawable render window counts.

import type { ContentBlock, JsonlEntryData } from '../../types/jsonl'

/** AssistantEntry draws a body unless every block is empty text/thinking. */
export function assistantHasVisibleContent(entry: JsonlEntryData): boolean {
  const content = entry.message?.content
  if (!Array.isArray(content)) return true
  return (content as ContentBlock[]).some((b) => {
    if (!b || !(b as { type?: string }).type) return false
    if (b.type === 'thinking') return !!(b.thinking && b.thinking.trim())
    if (b.type === 'text') return !!(b.text && b.text.trim())
    // tool_use, tool_result, and any NEW/unknown Anthropic block type render
    // something (ContentBlocks routes unknowns to a labelled fallback).
    return true
  })
}

/** SystemEntry's own null conditions. */
export function systemEntryDraws(entry: JsonlEntryData): boolean {
  const sub = entry.subtype
  if (sub === 'compact_boundary' || sub === 'api_error' || sub === 'model_fallback') return true
  if (sub === 'turn_duration') return false
  if (sub === 'init') {
    const errs = (entry as { mcp_server_errors?: unknown[] }).mcp_server_errors
    return Array.isArray(errs) && errs.length > 0
  }
  const content = entry.content
  return typeof content === 'string' && content.trim().length > 0
}

/** Text a compact-summary row shows; empty means it draws nothing. */
export function compactSummaryText(entry: JsonlEntryData): string {
  const content = entry.message?.content
  if (typeof content === 'string') return content.trim()
  if (Array.isArray(content)) {
    return (content as ContentBlock[])
      .filter((b) => b.type === 'text')
      .map((b) => b.text || '')
      .join('\n\n')
      .trim()
  }
  return ''
}

/** A tool group draws only when it wraps at least one call. */
export function toolGroupDraws(entry: JsonlEntryData): boolean {
  const group = (entry as { _groupEntries?: unknown[] })._groupEntries
  return Array.isArray(group) && group.length > 0
}
