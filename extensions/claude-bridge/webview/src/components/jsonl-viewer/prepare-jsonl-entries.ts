// Visibility/segment/interrupt/skip/api-error collapse helpers extracted
// from JsonlViewer.tsx (Sprint 5 / Stage E1). Pure functions — no rendering.

import { isInvisibleAssistantEntry, isToolResultEntry, isCaveatOnlyEntry, isVisibleAttachment, NON_RENDERING_ENTRY_TYPES } from './utils'
import type { JsonlEntryData, ContentBlock } from '../../types/jsonl'

export interface CompactSegmentRange { from: number; to: number }

/** Типы, для которых у JsonlEntry ЕСТЬ ветка рендера. Всё вне этого списка и
 *  вне NON_RENDERING_ENTRY_TYPES — новый тип CLI: скрываем (см. entryIsVisible),
 *  иначе он молча ест слоты render-окна, рисуя null. */
const RENDERABLE_ENTRY_TYPES = new Set([
  'user', 'assistant', 'system', 'attachment', 'tool_group',
])
const warnedUnknownTypes = new Set<string>()

/** UUID set of entries that fall in the active segment range. tool_group
 *  synthetics wrap real entries under `_groupEntries`; their own uuid
 *  (`tool-group-<orig>`) is NOT in `currentEntries` so we drop them in by
 *  the wrapped uuids only. */
export function buildUuidsInRange(
  entries: JsonlEntryData[],
  segs: CompactSegmentRange[],
  segIdx: number,
): Set<string> | null {
  if (segs.length === 0) return null
  const range = segs[segIdx]
  if (!range) return null
  const set = new Set<string>()
  for (let i = range.from; i < range.to; i++) {
    const u = (entries[i] as any)?.uuid
    if (typeof u === 'string') set.add(u)
  }
  return set
}

export function inActiveRange(
  e: JsonlEntryData,
  uuidsInRange: Set<string> | null,
  entries: JsonlEntryData[],
  range: CompactSegmentRange | undefined,
): boolean {
  if (!uuidsInRange) return true
  if (e.type === 'tool_group') {
    const grp = (e as any)._groupEntries as JsonlEntryData[] | undefined
    if (Array.isArray(grp)) {
      for (const g of grp) {
        const u = (g as any).uuid
        if (typeof u === 'string' && uuidsInRange.has(u)) return true
      }
      return false
    }
  }
  const u = (e as any).uuid
  if (typeof u === 'string') return uuidsInRange.has(u)
  // No uuid (legacy/synthetic without group) — fall back to identity
  // lookup, then to "show" so we don't disappear bona fide entries
  // silently.
  const idx = entries.indexOf(e)
  if (idx >= 0 && range) return idx >= range.from && idx < range.to
  return true
}

export interface VisibilityCtx {
  uuidsInRange: Set<string> | null
  range: CompactSegmentRange | undefined
  entries: JsonlEntryData[]
  toolResults: Map<string, any> | undefined
  isLatestSegment: boolean
  chainUuids: Set<string> | null
}

export function entryIsVisible(e: JsonlEntryData, ctx: VisibilityCtx): boolean {
  // MITM streaming entry. Use `!== undefined` (not `=== true`) so the brief
  // gap between `message_stop` (flips __streaming to false) and the JSONL
  // watcher delivering the canonical replacement does not flicker the
  // bubble. EXCEPT: side-quest streams (Haiku title generator, sub-agent,
  // classifier) are not written to JSONL by CLI at all — leaving them
  // visible turns the streaming stub into a permanent ghost (visible until
  // page reload). Hide them in the main viewer; the Subagent panel can show
  // them separately if needed.
  if (e.__streaming !== undefined) {
    if ((e as any).isSidechain) return false
    // A SETTLED synthetic turn — the CLI's "No response requested." after a
    // slash command, or an empty/thinking-only turn — must stay hidden even as
    // a streaming stub. Its canonical JSONL twin is filtered at line 98, so
    // surfacing the stub is a ghost bubble: it becomes the chat's visible
    // "last message" while the console (no terminal output for it) still shows
    // the real prior reply, and the two panels disagree. `isInvisibleAssistantEntry`
    // returns FALSE while `__streaming === true` (utils.ts:173), so a live stream
    // is never hidden mid-flight — only the settled (`__streaming === false`)
    // synthetic/empty stub is.
    if (e.type === 'assistant' && isInvisibleAssistantEntry(e)) return false
    return true
  }
  if (!inActiveRange(e, ctx.uuidsInRange, ctx.entries, ctx.range)) return false
  // Bookkeeping rows have no rendering path — counting them as "visible" let a
  // run of them eat the whole render window and blank the chat (see the list).
  if (NON_RENDERING_ENTRY_TYPES.has(e.type)) return false
  // НЕИЗВЕСТНЫЙ верхнеуровневый type: JsonlEntry рисует для него null, а тут
  // он считался видимым — новый служебный тип CLI молча ел слоты render-окна
  // (класс бага mode/stop_hook_summary, но для типов, которых ещё нет в
  // списках). Скрываем + один warn на тип, чтобы появление нового типа было
  // видно в консоли, а не в виде опустевшего чата.
  if (!RENDERABLE_ENTRY_TYPES.has(e.type)) {
    if (!warnedUnknownTypes.has(e.type)) {
      warnedUnknownTypes.add(e.type)
      console.warn(`[jsonl] unknown entry type "${e.type}" — hidden (no renderer)`)
    }
    return false
  }
  if (e.type === 'attachment') return isVisibleAttachment(e)
  // SYSTEM: mirror SystemEntry's OWN null conditions. There was no system branch
  // here at all, so `stop_hook_summary` — 928 across a real corpus, 757 in ONE
  // session — counted as visible while drawing nothing: the same bug as `mode`,
  // but keyed on `subtype`, which is why NON_RENDERING_ENTRY_TYPES can't catch
  // it. Note it reads `content`, NOT `message`: no system entry in 1,043 sampled
  // carries a `message` field, so the `!e.message` gates used elsewhere for this
  // are testing a field that never exists.
  if (e.type === 'system') {
    const sub = (e as { subtype?: string }).subtype
    // These three draw their own block regardless of content.
    if (sub === 'compact_boundary' || sub === 'api_error' || sub === 'away_summary') return true
    if (sub === 'turn_duration') return false // CLI timing bookkeeping
    const content = (e as { content?: unknown }).content
    if (typeof content !== 'string' || !content.trim()) return false
  }
  if (e.type === 'assistant' && isInvisibleAssistantEntry(e)) return false
  // Chain membership — orphans (cancelled prompts, dead /branch siblings)
  // are CLI-hidden, so we hide them too. Entries without uuid fall through.
  if (ctx.isLatestSegment && ctx.chainUuids && (e.type === 'user' || e.type === 'assistant')) {
    const u = (e as any).uuid
    if (typeof u === 'string' && !ctx.chainUuids.has(u)) return false
  }
  if (e.type === 'user') {
    if ((e as any).isMeta) return false
    if (isCaveatOnlyEntry(e)) return false
    if (isToolResultEntry(e) && ctx.toolResults && ctx.toolResults.size > 0) {
      const content = e.message?.content
      if (Array.isArray(content)) {
        const allMerged = content.every((b: any) =>
          b.type !== 'tool_result' || (b.tool_use_id && ctx.toolResults!.has(b.tool_use_id))
        )
        if (allMerged) return false
      }
    }
  }
  return true
}

export function isApiErrorEntry(e: JsonlEntryData): boolean {
  return e.type === 'system' && (e as any).subtype === 'api_error'
}

function textOfUserEntry(e: JsonlEntryData): string {
  if (e.type !== 'user') return ''
  const c = e.message?.content
  if (typeof c === 'string') return c.trim()
  if (Array.isArray(c)) {
    for (const b of c as ContentBlock[]) {
      if (b.type === 'text' && typeof b.text === 'string') return b.text.trim()
    }
  }
  return ''
}

export function isInterruptUserEntry(e: JsonlEntryData): boolean {
  const t = textOfUserEntry(e)
  return t === '[Request interrupted by user]' || t === '[Request interrupted by user for tool use]'
}

/** What AskUserWidget.tsx sends when the user clicks Skip on an
 *  AskUserQuestion elicitation. CLI surfaces it as a user-type entry,
 *  which would render as a bogus duplicate bubble — fold it into a
 *  "skipped" badge on the preceding user prompt instead. */
export function isSkippedUserEntry(e: JsonlEntryData): boolean {
  return textOfUserEntry(e) === '[Skipped by user]'
}

/** Tag the user-prompt entry that immediately precedes an
 *  [interrupt]/[skipped] marker so the renderer can show a badge on it. */
export function buildInterruptSkipFlags(merged: JsonlEntryData[]): {
  interruptFlags: Set<JsonlEntryData>
  skippedFlags: Set<JsonlEntryData>
} {
  const interruptFlags = new Set<JsonlEntryData>()
  const skippedFlags = new Set<JsonlEntryData>()
  let lastUser: JsonlEntryData | null = null
  for (const e of merged) {
    if (e.type !== 'user') continue
    if (isInterruptUserEntry(e)) {
      if (lastUser) interruptFlags.add(lastUser)
      continue
    }
    if (isSkippedUserEntry(e)) {
      if (lastUser) skippedFlags.add(lastUser)
      continue
    }
    if ((e as any).isMeta) continue
    if (isCaveatOnlyEntry(e)) continue
    lastUser = e
  }
  return { interruptFlags, skippedFlags }
}
