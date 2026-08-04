import type { JsonlEntryData, ContentBlock } from '../../types/jsonl'
import { stripMcpPrefix } from '../../lib/mcp-tool-name'

/** A ShowWidget renders as its own clean visual — it must never be folded into a
 *  collapsed tool burst, so an entry carrying one is kept standalone. */
function hasWidgetTool(entry: JsonlEntryData): boolean {
  const content = entry.message?.content
  if (!Array.isArray(content)) return false
  return (content as ContentBlock[]).some(
    (b) => b.type === 'tool_use' && stripMcpPrefix(b.name || '') === 'ShowWidget',
  )
}

export function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

export function formatTokens(n: number): string {
  if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M'
  if (n >= 1000) return (n / 1000).toFixed(1) + 'K'
  return String(n)
}

// Компиляция RegExp на каждый вызов гонялась ×16 на рендер CliXmlContent
// (аудит #70 B6); имён тегов — фиксированная горстка, кэшируем по имени.
const extractTagCache = new Map<string, RegExp>()

export function extractTag(text: string, tagName: string): string | null {
  let re = extractTagCache.get(tagName)
  if (!re) {
    re = new RegExp(`<${tagName}>([\\s\\S]*?)</${tagName}>`, 'i')
    extractTagCache.set(tagName, re)
  }
  const m = text.match(re)
  return m ? m[1].trim() : null
}

export function hasCliXmlTags(text: string): boolean {
  return /<(?:command-name|command-args|command-message|local-command-caveat|local-command-stdout|local-command-stderr|system-reminder|claude-mem-context|claude_background_info|fast_mode_info|env|task-notification|user_message|environment[_-]details|antml:thinking|error-details|teammate-message)[\s>]/i.test(text)
}

export function extractTeammateMessages(text: string): Array<{ id: string; color?: string; body: string; parsed?: any }> {
  const results: Array<{ id: string; color?: string; body: string; parsed?: any }> = []
  const re = /<teammate-message\s+teammate_id="([^"]*)"(?:\s+color="([^"]*)")?[^>]*>([\s\S]*?)<\/teammate-message>/g
  let m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    const body = m[3].trim()
    let parsed: any = undefined
    try {
      const obj = JSON.parse(body)
      if (obj && typeof obj === 'object' && obj.type) parsed = obj
    } catch { /* not JSON */ }
    results.push({ id: m[1], color: m[2] || undefined, body, parsed })
  }
  return results
}

/** Entry types the CLI writes as bookkeeping — they have NO rendering path in
 *  JsonlEntry (titles/agent-names are folded into the header chrome; the rest are
 *  internal markers). Every layer must agree on this list, hence one constant.
 *
 *  It emptied a whole chat once: `entryIsVisible` hid only the first four while
 *  this file's burst-grouping already knew about all eight, so hundreds of
 *  consecutive `ai-title` rows (the CLI re-emits one per turn) counted as
 *  VISIBLE, filled the entire 150-entry render window, and each rendered null →
 *  a blank panel with every real message pushed behind "N earlier messages —
 *  scroll up to load". The diagnostic dump named it: storeCount 1059 but only
 *  181 uuids — 878 uuid-less bookkeeping rows. */
// THE list. `JsonlEntry` renders exactly user / assistant / system / attachment
// / tool_group — every other type returns null, so every other type MUST be in
// here. When one isn't, it counts as "visible" while drawing nothing: it eats
// the render window (blank chat), splits tool bursts in two, and inflates the
// segment-tab counts.
//
// `mode` was the one that got away — 542 of them in a real session, sitting
// right next to last-prompt/ai-title/permission-mode. It produced exactly those
// three symptoms, and the user hit all three.
//
// The last four came from compact-segments.ts, which had grown its OWN
// hardcoded copy of this list — one that knew todo-snapshot/compact-summary/
// tool-call-error but not ai-title/summary. So the segment pill and the viewer
// disagreed about what counts. One list now, imported by both.
export const NON_RENDERING_ENTRY_TYPES: ReadonlySet<string> = new Set([
  'last-prompt',
  'permission-mode',
  'queue-operation',
  'file-history-snapshot',
  'ai-title',
  'custom-title',
  'agent-name',
  'summary',
  'mode',
  'todo-snapshot',
  'compact-summary',
  'tool-call-error',
])

/** Does an `attachment` row actually DRAW anything?
 *
 *  Most attachments are housekeeping only the model sees (task_reminder,
 *  hook_non_blocking_error, deferred tool lists, …). Exactly two are
 *  user-visible — and only when they carry their payload: a queued_command with
 *  a non-blank prompt, an edited_text_file with a filename. JsonlEntry returns
 *  null for everything else.
 *
 *  This predicate existed in FOUR places and one of them had drifted: the burst
 *  continuer in groupConsecutiveToolCalls checked only `att.type`, so an empty
 *  queued_command read as visible, ENDED the tool burst, and drew nothing — one
 *  burst rendered as two. One definition now; the others call it.
 */
export function isVisibleAttachment(entry: JsonlEntryData): boolean {
  const att = (entry as { attachment?: { type?: string; prompt?: unknown; filename?: unknown } }).attachment
  if (att?.type === 'queued_command') return typeof att.prompt === 'string' && att.prompt.trim().length > 0
  if (att?.type === 'edited_text_file') return typeof att.filename === 'string'
  return false
}

export function isToolResultEntry(entry: JsonlEntryData): boolean {
  const content = entry.message?.content
  if (!Array.isArray(content)) return false
  return (content as ContentBlock[]).some((block) => block.type === 'tool_result')
}

/**
 * True when an assistant entry is "tool-only" — contains at least one
 * tool_use block and no visible text or thinking. These are the entries
 * that the UI pipeline usually renders as the small blue/green "🔧 Tool"
 * pills, and they're the cluttery ones the user wants to collapse into
 * a single group block.
 */
export function isToolOnlyAssistantEntry(entry: JsonlEntryData): boolean {
  if (entry.type !== 'assistant') return false
  const content = entry.message?.content
  if (!Array.isArray(content)) return false
  let hasTool = false
  for (const b of content as ContentBlock[]) {
    if (b.type === 'tool_use') { hasTool = true; continue }
    if (b.type === 'text' && b.text && b.text.trim()) return false
    // Thinking blocks are invisible in the current viewer — they don't add
    // a visual line, so an entry "thinking + tool_use" still reads to the
    // user as tool-only and should join the group.
  }
  return hasTool
}

/**
 * True when an assistant entry has nothing visible — only thinking blocks,
 * empty text, or no content at all. AssistantEntry filters these out of
 * render, but they still sit in the entries array and would otherwise break
 * a tool burst chain (e.g. Claude "thinks" between two tool_use turns).
 */
/** CLI-synthesised strings that should never render as speech bubbles.
 *  Mirrors `SYNTHETIC_MESSAGES` in utils/messages.ts:302 plus the
 *  NO_CONTENT_MESSAGE placeholder and the missing-tool_result stub —
 *  they all appear in JSONL streams for internal bookkeeping but the CLI
 *  itself hides them from screen. */
export const SYNTHETIC_USER_TEXTS: Set<string> = new Set([
  '[Request interrupted by user]',
  '[Request interrupted by user for tool use]',
  '[Skipped by user]',
  'No response requested.',
  '(no content)',
  '[Tool result missing due to internal error]',
])
/** Prefixes that mark CLI-synthesised content (not a real user turn).
 *  The full strings live in utils/messages.ts:210-221 — each one is model-
 *  facing guidance text that gets bundled into a user message after a
 *  rejection/plan-reject, so it must not show up as a user bubble. */
export const SYNTHETIC_USER_PREFIXES: readonly string[] = [
  "The user doesn't want to take this action right now.",
  "The user doesn't want to proceed with this tool use.",
  'Permission for this tool use was denied.',
  'Permission to use ', // AUTO_REJECT_MESSAGE / DONT_ASK_REJECT_MESSAGE
  'The agent proposed a plan that was rejected by the user.',
]
export function isSyntheticUserText(raw: string): boolean {
  const t = raw.trim()
  if (!t) return false
  if (SYNTHETIC_USER_TEXTS.has(t)) return true
  for (const p of SYNTHETIC_USER_PREFIXES) if (t.startsWith(p)) return true
  return false
}

export function isInvisibleAssistantEntry(entry: JsonlEntryData): boolean {
  if (entry.type !== 'assistant') return false
  // Streaming entries are always considered visible — they MAY temporarily
  // have empty content while message_start fired but no blocks yet.
  if (entry.__streaming === true) return false
  const content = entry.message?.content
  if (!Array.isArray(content)) return false
  for (const b of content as ContentBlock[]) {
    if (b.type === 'tool_use') return false
    if (b.type === 'tool_result') return false
    if (b.type === 'text' && b.text) {
      const trimmed = b.text.trim()
      if (!trimmed) continue
      // CLI hides these synthetic assistant texts in AssistantTextMessage
      // .tsx:75 (NO_RESPONSE_REQUESTED) and via isEmptyMessageText for
      // NO_CONTENT_MESSAGE (utils/messages.ts:2753). Mirror the full set
      // so the ghost bubbles don't leak into our viewer.
      if (SYNTHETIC_USER_TEXTS.has(trimmed)) continue
      return false
    }
    // Thinking blocks with actual content — no longer hidden. Collapsed
    // ThinkingBlock.tsx shows a single-line teaser; full thought expands
    // on click. Without surfacing these, pure-thinking turns (extended
    // thinking mode with no text/tool follow-up yet) were invisible.
    if (b.type === 'thinking' && b.thinking && b.thinking.trim()) return false
  }
  return true
}

/** Extract tool_use names from an assistant entry (for tallying). */
export function toolNamesInEntry(entry: JsonlEntryData): string[] {
  const content = entry.message?.content
  if (!Array.isArray(content)) return []
  const names: string[] = []
  for (const b of content as ContentBlock[]) {
    if (b.type === 'tool_use' && typeof (b as any).name === 'string') {
      names.push((b as any).name)
    }
  }
  return names
}

/**
 * Group runs of 2+ consecutive tool-only assistant entries (plus their
 * immediately-following user tool_result entries) into a single synthetic
 * "tool_group" entry. The UI renders it as a collapsed summary with tool
 * counts and an "Expand" button that restores the original view inline.
 *
 * Why 2+: even two in a row already duplicate the same visual block, and
 * users want a tidy collapsed view whenever tools fire back-to-back.
 */
/**
 * Split an assistant message that carries BOTH prose and tool_use into the
 * bubble part and the tool part; null when there's nothing to split.
 *
 * The model routinely narrates before it acts ("Now I'll rebuild:" + a Bash in
 * the same message). Such an entry is not tool-ONLY, so it could never start a
 * burst: its tool rendered inline in the prose bubble while the NEXT, tool-only
 * call got a group to itself — the "1 tool call · Bash ×1" next to an ungrouped
 * Bash. Whether a call joined the group came down to whether the model happened
 * to type a sentence first, which is exactly the inconsistency users see.
 *
 * Both halves KEEP the original uuid: segment membership (buildUuidsInRange)
 * and chain membership are resolved by uuid against the ungrouped entries, so a
 * synthesised uuid here would drop the group out of its segment entirely. They
 * don't collide as render keys — the group's own key is `tool-group-<uuid>`.
 * Safe to share `message.id` too: this runs on the render copy, never the store.
 */
function splitProseFromTools(
  e: JsonlEntryData,
): { prose: JsonlEntryData; tools: JsonlEntryData } | null {
  if (e.type !== 'assistant') return null
  const content = e.message?.content
  if (!Array.isArray(content)) return null
  const blocks = content as ContentBlock[]
  const hasTool = blocks.some((b) => b.type === 'tool_use')
  const hasText = blocks.some((b) => b.type === 'text' && b.text && b.text.trim())
  if (!hasTool || !hasText) return null
  const clone = (keep: (b: ContentBlock) => boolean): JsonlEntryData =>
    ({ ...e, message: { ...e.message, content: blocks.filter(keep) } }) as JsonlEntryData
  return {
    // Thinking rides with the prose: it belongs to what the model SAID, and
    // leaving it here keeps the tool half tool-only.
    prose: clone((b) => b.type !== 'tool_use'),
    tools: clone((b) => b.type === 'tool_use'),
  }
}

export function groupConsecutiveToolCalls(entries: JsonlEntryData[]): JsonlEntryData[] {
  // Wrap a tool burst around even a single tool call so the user always
  // gets the consistent burst chrome (header + collapsible body) instead
  // of a bare assistant entry whenever the model fires one tool. Was
  // previously gated at ≥2 to keep one-shot Read/Bash entries minimal,
  // but inconsistency hurt UX more than the saved vertical space.
  const MIN_GROUP = 1
  const out: JsonlEntryData[] = []
  let i = 0
  while (i < entries.length) {
    const e = entries[i]
    // Streaming entry — never group while live; user wants to see it
    // standalone with cursor + partial blocks. Once finalized (JSONL flush
    // replaces it) the next group pass will absorb it normally.
    if (e.__streaming === true) {
      out.push(e)
      i++
      continue
    }
    // Prose + tool_use in one message: the prose becomes its own bubble and the
    // tool heads the burst, so grouping no longer depends on whether the model
    // narrated first. Nothing is hidden — the prose renders exactly as before,
    // just without the tool welded inside it.
    const split = splitProseFromTools(e)
    const head = split ? split.tools : e
    // A widget entry renders standalone (clean visual), never as burst chrome.
    if (isToolOnlyAssistantEntry(head) && !hasWidgetTool(head)) {
      if (split) out.push(split.prose)
      const groupEntries: JsonlEntryData[] = [head]
      let j = i + 1
      while (j < entries.length) {
        const n = entries[j]
        // Continuers — strictly part of the tool-call protocol or UI-invisible.
        // Anything that would render a visible bubble (real user message,
        // visible assistant text) ends the burst so it renders standalone.
        if (n.type === 'user' && isToolResultEntry(n)) { groupEntries.push(n); j++; continue }
        if (isToolOnlyAssistantEntry(n) && !hasWidgetTool(n)) { groupEntries.push(n); j++; continue }
        if (isInvisibleAssistantEntry(n)) { groupEntries.push(n); j++; continue }
        // User entries that only contain noise wrappers (system-reminder,
        // local-command-caveat, environment_details, etc.) are invisible
        // in the viewer and must not break the burst.
        if (n.type === 'user' && isCaveatOnlyEntry(n)) { groupEntries.push(n); j++; continue }
        // Bare system entries with no message/content render nothing.
        if (n.type === 'system' && !n.message) { groupEntries.push(n); j++; continue }
        // Transient api_error retry banners (ECONNRESET/ConnectionRefused
        // back-off retries) are tool-call plumbing noise, not conversation —
        // fold them into the burst instead of splitting it.
        if (n.type === 'system' && n.subtype === 'api_error') { groupEntries.push(n); j++; continue }
        // Bookkeeping rows render nothing, so they must not split a tool burst.
        if (NON_RENDERING_ENTRY_TYPES.has(n.type)) { groupEntries.push(n); j++; continue }
        // Attachment entries — invisible except for `queued_command` and
        // `edited_text_file` (those render their own bubble). Everything
        // else (task_reminder, raw image metadata, …) must not split a burst.
        if (n.type === 'attachment' && !isVisibleAttachment(n)) { groupEntries.push(n); j++; continue }
        break
      }
      const toolCount = groupEntries.filter(x => x.type === 'assistant' && isToolOnlyAssistantEntry(x)).length
      if (toolCount >= MIN_GROUP) {
        const synthetic: JsonlEntryData = {
          type: 'tool_group',
          timestamp: groupEntries[0].timestamp,
          uuid: `tool-group-${groupEntries[0].uuid ?? i}`,
          _groupEntries: groupEntries,
        }
        out.push(synthetic)
        i = j
        continue
      }
    }
    out.push(e)
    i++
  }
  return out
}

/** Fuse consecutive assistant entries that share the same `message.id` into
 *  one entry by concatenating their `message.content` block arrays.
 *
 *  Why: Claude CLI v2.1.x splits a single model turn into separate JSONL
 *  lines per content block — thinking, text, tool_use each get their own
 *  entry with a fresh `uuid` but the same `message.id`. Without this step
 *  AssistantEntry's `isInvisibleAssistantEntry` check eats the leading
 *  thinking-only entry (empty thinking text + signature only) and the
 *  trailing chain walk can orphan the text-only entry — net effect the
 *  user sees nothing, even though the model emitted real text. Mirrors
 *  the same step in `packages/jsonl-viewer-core/src/utils.ts` so the
 *  pure pipeline (offline tests, `processJsonl`) stays in sync. */
export function mergeAssistantEntriesByMessageId(entries: JsonlEntryData[]): JsonlEntryData[] {
  if (entries.length === 0) return entries
  const out: JsonlEntryData[] = []
  // Cleared on any non-assistant entry so distantly-separated turns that
  // happen to reuse a message.id don't accidentally fuse.
  const idxByMsgId = new Map<string, number>()
  for (const e of entries) {
    if (e.type !== 'assistant') {
      out.push(e)
      idxByMsgId.clear()
      continue
    }
    const msgId = e.message?.id
    if (!msgId) { out.push(e); continue }
    const targetIdx = idxByMsgId.get(msgId)
    if (targetIdx === undefined) {
      out.push(e)
      idxByMsgId.set(msgId, out.length - 1)
      continue
    }
    const target = out[targetIdx]!
    const targetContent = Array.isArray(target.message?.content) ? target.message!.content : []
    const incomingContent = Array.isArray(e.message?.content) ? e.message!.content : []
    out[targetIdx] = {
      ...target,
      ...e,
      message: {
        ...target.message,
        ...e.message,
        content: targetContent.concat(incomingContent),
      },
      // ...e spread adopts the latest uuid so chain walks via `parentUuid`
      // continue to resolve to the merged bubble.
    } as JsonlEntryData
  }
  return out
}

/**
 * Merge consecutive user entries with CLI XML tags (command + stdout/stderr).
 * e.g. entry1: "<command-name>/context</command-name>" + entry2: "<local-command-stdout>...</local-command-stdout>"
 * become a single entry with merged content.
 */
export function mergeConsecutiveCliEntries(entries: JsonlEntryData[]): JsonlEntryData[] {
  if (entries.length === 0) return entries
  const result: JsonlEntryData[] = []

  for (let i = 0; i < entries.length; i++) {
    const entry = entries[i]
    const text = getEntryText(entry)

    // Check if this is a command entry (has <command-name>) — can be user or system type
    if ((entry.type === 'user' || entry.type === 'system') && text && /<command-name>/i.test(text)) {
      let merged = text
      // Look ahead: merge following entries of same type with same timestamp (within 2s)
      while (i + 1 < entries.length) {
        const next = entries[i + 1]
        if (next.type !== entry.type) break
        const nextText = getEntryText(next)
        if (!nextText) break
        // Check timestamp proximity
        if (entry.timestamp && next.timestamp) {
          const diff = Math.abs(new Date(entry.timestamp).getTime() - new Date(next.timestamp).getTime())
          if (diff > 2000) break
        }
        // Don't merge if the next entry is also a command
        if (/<command-name>/i.test(nextText)) break
        merged += '\n' + nextText
        i++
      }
      if (merged !== text) {
        const mergedEntry = entry.type === 'system'
          ? { ...entry, content: merged, _merged: true } as any
          : { ...entry, message: { ...entry.message, content: merged }, _merged: true } as any
        result.push(mergedEntry)
      } else {
        result.push(entry)
      }
    } else {
      result.push(entry)
    }
  }
  return result
}

function getEntryText(entry: JsonlEntryData): string {
  // System entries store content directly, user/assistant in message.content
  if (entry.type === 'system' && typeof entry.content === 'string') {
    return entry.content
  }
  const content = entry.message?.content
  if (typeof content === 'string') return content
  if (Array.isArray(content)) {
    return (content as ContentBlock[])
      .filter(b => b.type === 'text')
      .map(b => b.text || '')
      .join('\n')
  }
  return ''
}

// The tool-result map and its incremental cache moved to lib/tool-result-cache
// once the Files panel needed the same data: it was rebuilding an identical map
// over full history on every store change, 26ms per arriving record on a real
// 31k-entry session. Re-exported here so viewer call sites read unchanged.
export type { ToolResultMap } from '../../lib/tool-result-cache'
export { buildToolResultMap, foldToolResultsInto } from '../../lib/tool-result-cache'

/** Check if a user entry is actually a teammate message (not real user input) */
export function isTeammateMessageEntry(entry: JsonlEntryData): boolean {
  if (entry.type !== 'user') return false
  const content = entry.message?.content
  if (typeof content === 'string') {
    const stripped = content.replace(/<teammate-message[\s\S]*?<\/teammate-message>/gi, '').trim()
    return stripped === '' && content.includes('<teammate-message')
  }
  if (Array.isArray(content)) {
    const texts = (content as ContentBlock[]).filter(b => b.type === 'text').map(b => b.text || '')
    const joined = texts.join('')
    const stripped = joined.replace(/<teammate-message[\s\S]*?<\/teammate-message>/gi, '').trim()
    return stripped === '' && joined.includes('<teammate-message')
  }
  return false
}

// Модульный уровень: литерал в теле функции пересоздавал гигантский RegExp
// на КАЖДЫЙ вызов, а isCaveatOnlyEntry зовётся O(сегмента) раз на каждый
// пересчёт модели (аудит #70 B6). Глобальный флаг безопасен: используется
// только в .replace.
const NOISE_TAGS = /<(?:local-command-caveat|system-reminder|environment[_-]details|claude-mem-context|claude_background_info|fast_mode_info|env|task-notification|command-message|command-args|command-name|local-command-stdout|local-command-stderr)[\s>][\s\S]*?<\/(?:local-command-caveat|system-reminder|environment[_-]details|claude-mem-context|claude_background_info|fast_mode_info|env|task-notification|command-message|command-args|command-name|local-command-stdout|local-command-stderr)>/g

export function isCaveatOnlyEntry(entry: JsonlEntryData): boolean {
  // Returns true for user entries that contain only noise wrappers (local
  // command caveats, system-reminder blocks, environment_details, etc.) — the
  // model reads them but they clutter the chat log.
  // ALSO catches Skill bodies: CLI re-injects SKILL.md content as a synthetic
  // user turn starting with "Base directory for this skill: …" — same logical
  // role as tool_result; folds into the tool burst.
  const check = (text: string): boolean => {
    const trimmed = text.trim()
    if (trimmed.startsWith('Base directory for this skill:')) return true
    const stripped = text.replace(NOISE_TAGS, '').trim()
    return stripped === '' && /<(?:local-command-caveat|system-reminder|environment[_-]details|claude-mem-context|claude_background_info|fast_mode_info|env|task-notification|command-)/i.test(text)
  }
  const content = entry.message?.content
  if (typeof content === 'string') return check(content)
  if (Array.isArray(content)) {
    const joined = (content as ContentBlock[]).filter((b) => b.type === 'text').map((b) => b.text || '').join('')
    return check(joined)
  }
  return false
}

const MCP_PREFIX = 'mcp__'

/** `mcp__<server>__<tool>` → `<tool>`. Split on the `__` SEPARATOR, not on the
 *  first underscore, so a server named `my_server` doesn't eat half the tool
 *  name. Shared by both tool-group renderers — it used to be copy-pasted into
 *  each, with the offset written as `MCP_PREFIX.length` in one and a bare `5`
 *  in the other. */
export function prettyToolName(name: string): string {
  const idx = name.indexOf('__', MCP_PREFIX.length)
  return name.startsWith(MCP_PREFIX) && idx > MCP_PREFIX.length ? name.slice(idx + 2) : name
}
