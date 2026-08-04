// ============================================================================
// Compact-segment index — built server-side from the authoritative transcript.
//
// The server owns the COMPLETE file, so this is the one reliable source for
// "every compact segment + how many messages each holds". The client's local
// mirror is a bounded tail and cannot see older segments — that is why the
// strip used to drop them for long sessions, and why its counts (read off the
// mirror) were inflated by resume-duplicated + protocol-noise records.
//
// `counts` are VISIBLE records only. A raw line count runs ~10x larger,
// dominated by permission-mode/last-prompt/mode bookkeeping the chat never
// draws (a 3 625-line session counted 19 411 across its segments). The filter
// mirrors the webview's `entryIsVisible` (prepare-jsonl-entries.ts) minus the
// streaming/range/chain concerns that only exist client-side. It counts visible
// ENTRIES, not merged bubbles — a tool burst the chat collapses into one card
// still counts as its N entries, so a badge can read slightly higher than the
// opened segment's bubble count. That trade keeps this a single linear pass
// with no burst-grouping logic duplicated from the renderer.
// ============================================================================

import type { JsonlEntry, ContentBlock } from '../../shared/jsonl-types'

export interface SegmentIndex {
  /** compact_boundary timestamps, in file (chronological) order. */
  boundaries: { ts: string }[]
  /** One MORE entry than `boundaries`: counts[0] is the original pre-compaction
   *  segment; counts[k+1] is the segment starting AFTER boundaries[k]. Boundary
   *  rows themselves are dividers and are never counted. */
  counts: number[]
}

// Bookkeeping types the CLI emits per turn/tick that draw nothing in the chat.
// INTENTIONALLY a superset of the webview's NON_RENDERING_ENTRY_TYPES (utils.ts):
// the two live in different packages (server vs webview) with no shared runtime
// module to import, so this cannot be the same array. The extra members here
// (agent-color, tag, worktree-state, …) all render `null` in the webview anyway,
// so excluding them from the count is additively-correct, never wrong. A future
// divergence only mis-counts a badge — it can't affect what renders. If strict
// sync is ever needed, add a cross-package diff-test rather than sharing the literal.
const NON_RENDERING = new Set<string>([
  'last-prompt', 'permission-mode', 'queue-operation', 'file-history-snapshot',
  'ai-title', 'custom-title', 'agent-name', 'summary', 'mode', 'todo-snapshot',
  'compact-summary', 'tool-call-error', 'attribution-snapshot', 'agent-color',
  'agent-setting', 'tag', 'task-summary', 'pr-link', 'worktree-state',
  'content-replacement', 'progress', 'speculation-accept',
])

// CLI-synthesised placeholder texts that never render as a bubble (mirrors
// SYNTHETIC_USER_TEXTS). A lone-synthetic turn is invisible.
const SYNTHETIC_TEXTS = new Set<string>([
  '[Request interrupted by user]', '[Request interrupted by user for tool use]',
  '[Skipped by user]', 'No response requested.', '(no content)',
  '[Tool result missing due to internal error]',
])

// User turns whose text is ONLY CLI noise wrappers (slash-command echoes, local
// caveats, system-reminders, injected skill bodies) — the model reads them, the
// chat hides them. Mirrors isCaveatOnlyEntry (utils.ts).
const NOISE_TAG =
  '(?:local-command-caveat|system-reminder|environment[_-]details|claude-mem-context|claude_background_info|fast_mode_info|env|task-notification|command-message|command-args|command-name|local-command-stdout|local-command-stderr)'
const NOISE_TAGS_RE = new RegExp(`<${NOISE_TAG}[\\s>][\\s\\S]*?<\\/${NOISE_TAG}>`, 'g')
const NOISE_LEAD_RE = new RegExp(`<${NOISE_TAG}`, 'i')

function firstUserText(content: string | ContentBlock[] | undefined): string {
  if (typeof content === 'string') return content
  if (Array.isArray(content)) {
    for (const b of content) if (b.type === 'text' && typeof b.text === 'string') return b.text
  }
  return ''
}

function isCaveatOnly(text: string): boolean {
  if (!text) return false
  if (text.trim().startsWith('Base directory for this skill:')) return true
  const stripped = text.replace(NOISE_TAGS_RE, '').trim()
  return stripped === '' && NOISE_LEAD_RE.test(text)
}

function isToolResultUser(content: string | ContentBlock[] | undefined): boolean {
  return Array.isArray(content) && content.some((b) => b.type === 'tool_result')
}

function isVisibleAssistant(e: JsonlEntry): boolean {
  const content = e.message?.content
  // Non-array (plain string) assistant content renders as-is — matches the
  // client's `isInvisibleAssistantEntry` returning false for it.
  if (!Array.isArray(content)) return typeof content === 'string' && content.trim().length > 0
  for (const b of content) {
    if (b.type === 'tool_use' || b.type === 'tool_result') return true
    if (b.type === 'text' && b.text.trim() && !SYNTHETIC_TEXTS.has(b.text.trim())) return true
    if (b.type === 'thinking' && b.thinking.trim()) return true
  }
  return false
}

function isVisibleAttachment(e: JsonlEntry): boolean {
  const att = e.attachment
  if (att?.type === 'queued_command') return typeof att.prompt === 'string' && att.prompt.trim().length > 0
  if (att?.type === 'edited_text_file') return typeof att.filename === 'string'
  return false
}

/** True when an entry draws a bubble/divider in the chat (so the strip badge
 *  matches roughly what opening the segment shows). compact_boundary rows are
 *  excluded here — they are the segment DIVIDERS, counted structurally, not as
 *  content. */
export function isVisibleEntry(e: JsonlEntry): boolean {
  const t = e.type as string
  if (NON_RENDERING.has(t)) return false
  if (t === 'attachment') return isVisibleAttachment(e)
  if (t === 'system') {
    const sub = (e as { subtype?: string }).subtype
    if (sub === 'compact_boundary') return false
    if (sub === 'api_error' || sub === 'away_summary') return true
    if (sub === 'turn_duration') return false
    return typeof e.content === 'string' && e.content.trim().length > 0
  }
  if (t === 'assistant') return isVisibleAssistant(e)
  if (t === 'user') {
    if (e.isMeta) return false
    const text = firstUserText(e.message?.content).trim()
    if (SYNTHETIC_TEXTS.has(text)) return false
    if (isToolResultUser(e.message?.content)) return false
    if (isCaveatOnly(firstUserText(e.message?.content))) return false
    return true
  }
  return true
}

/** Build the index from the fully-parsed transcript and the file-order indices
 *  of its compact_boundary rows (the watcher already collects both during
 *  replay, so this is a single extra linear pass, no re-read).
 *
 *  `boundaries` and `counts` are built in lock-step so the invariant the client
 *  relies on always holds: `counts.length === boundaries.length + 1`, counts[0]
 *  is the pre-first-boundary segment, counts[k+1] the segment after
 *  boundaries[k]. A compact_boundary with no usable `timestamp` (optional in the
 *  type, so reachable) has no place on the ts axis and is NOT emitted — its
 *  segment folds into the previous slot rather than desyncing the pairing, which
 *  would otherwise mislabel every later segment and load the wrong ts range. */
export function buildSegmentIndex(all: JsonlEntry[], boundaryIdx: number[]): SegmentIndex {
  const countRange = (from: number, to: number): number => {
    let n = 0
    for (let i = from; i < to; i++) if (isVisibleEntry(all[i]!)) n++
    return n
  }
  const boundaries: { ts: string }[] = []
  const counts: number[] = []
  // Running count for the slot currently being accumulated — starts as the
  // original conversation (everything before the first boundary).
  let current = countRange(0, boundaryIdx.length ? boundaryIdx[0]! : all.length)
  for (let k = 0; k < boundaryIdx.length; k++) {
    const from = boundaryIdx[k]! + 1 // skip the boundary divider row itself
    const to = k + 1 < boundaryIdx.length ? boundaryIdx[k + 1]! : all.length
    const segCount = countRange(from, to)
    const ts = String(all[boundaryIdx[k]!]?.timestamp ?? '')
    if (ts) {
      counts.push(current) // close the slot this boundary ends
      boundaries.push({ ts })
      current = segCount // open the slot this boundary starts
    } else {
      current += segCount // ts-less divider: no split — merge into the current slot
    }
  }
  counts.push(current) // the final (newest) segment
  return { boundaries, counts }
}
