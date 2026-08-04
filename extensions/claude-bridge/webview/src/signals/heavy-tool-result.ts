// Offload the BYTES of a fat tool_result out of the resident store at ingest.
//
// The chat NEVER renders more than 2000 chars of a text tool_result (JsonlToolUse
// and JsonlToolResult both hard-slice at 2000, expanded or not) — yet the raw
// store held the WHOLE body of every result, and a session full of big file reads
// / grep / bash dumps is what pushed the shared renderer past its ~4GB heap and
// killed it with OUT_OF_MEMORY (10.6k entries ≈ 3.9GB, ~370KB each). Because the
// UI can't show past 2000 chars anyway, truncating the in-memory copy to a
// generous prefix loses nothing the user can see; the full body stays on disk
// (export reads the JSONL there, not this store). The `_fullLen` we stash keeps
// the "N chars" label honest.
//
// IMAGES ARE NEVER TOUCHED: a base64 image payload truncated mid-string corrupts
// the <img> src. We detect image-shaped strings with cheap prefix/substring
// checks (no JSON.parse at ingest) and leave them whole.
import type { JsonlEntryData, ContentBlock } from '../types/jsonl'

// Only bodies larger than this are trimmed; KEEP_PREFIX is what stays resident.
// 16KB is 8× the 2000 chars the UI shows — headroom if that cap is ever raised,
// while still cutting a multi-hundred-KB result to a fraction.
const TRIM_THRESHOLD = 16_384
const KEEP_PREFIX = 16_384

/** A string that IS (or wraps) a base64 image — must never be truncated. Cheap
 *  checks only; the real parse happens later in ToolResultBlock, on screen. */
function looksLikeImage(s: string): boolean {
  const t = s.trimStart()
  if (t.startsWith('data:image/')) return true
  if ((t.startsWith('{') || t.startsWith('[')) && t.includes('"type":"image"')) return true
  return false
}

/** Prefix + original length for an over-threshold text body; null to leave as-is. */
function trimText(s: string): { text: string; full: number } | null {
  if (s.length <= TRIM_THRESHOLD || looksLikeImage(s)) return null
  return { text: s.slice(0, KEEP_PREFIX), full: s.length }
}

/** Truncate the heavy text bodies of an entry's tool_result blocks IN PLACE.
 *  Idempotent (a block already carrying `_fullLen` is skipped) and image-safe.
 *  Returns the same entry for convenient chaining. */
export function trimHeavyToolResults(entry: JsonlEntryData): JsonlEntryData {
  const content = entry.message?.content
  if (!Array.isArray(content)) return entry
  for (const block of content as ContentBlock[]) {
    if (!block || block.type !== 'tool_result' || block._fullLen !== undefined) continue
    const c = block.content
    if (typeof c === 'string') {
      const r = trimText(c)
      if (r) { block.content = r.text; block._fullLen = r.full }
    } else if (Array.isArray(c)) {
      // Structured content: trim each large TEXT item, sum the true lengths,
      // leave image items alone. Only stamp `_fullLen` if something was trimmed.
      let full = 0
      let trimmed = false
      for (const item of c as ContentBlock[]) {
        if (typeof item?.text !== 'string') continue
        const r = trimText(item.text)
        if (r) { item.text = r.text; full += r.full; trimmed = true }
        else full += item.text.length
      }
      if (trimmed) block._fullLen = full
    }
  }
  return entry
}
