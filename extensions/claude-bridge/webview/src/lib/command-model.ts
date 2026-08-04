// Pure model behind the command API: the state SHAPE, the quiescence key and
// entry flattening — no signal imports.
//
// Kept apart from `command-state.ts` (which reads live signals) on purpose: the
// signal tree transitively pulls `@bridge/storage`, which the repo-wide vitest
// config does not alias, so a test importing it fails to load there. Same reason
// `signals/replay-progress` lives in its own module.

export interface BridgeState {
  activeTabId: string | null
  /** What the viewer is actually SHOWING. Differing from `activeTabId` means the
   *  cover is up and the composer refuses to send. */
  boundTabId: string | null
  mcpLoading: boolean
  promptReady: boolean
  /** A turn is in flight (the activity spinner). */
  working: boolean
  /** Non-null only while history is downloading. */
  replayPct: number | null
  entryCount: number
  segmentCount: number
  activeSegment: number
  /** Viewing an archived (out-of-window) compact segment rather than the live tail. */
  archived: boolean
  /** The view is mid-rebuild: history streaming, tab not yet bound, or MCP still
   *  connecting. Reads of `entries()`/`segments()` are NOT trustworthy here. */
  settling: boolean
  /** Nothing can be sent right now: settling, mid-turn, or no prompt. */
  busy: boolean
  /** Messages shown as pending in the queue widget. */
  queued: number
}

/** The fields that must hold still for the view to count as settled. Excludes
 *  `replayPct` (it ticks by design) — its `null`ness is already in `settling`. */
export function quiescenceKey(s: BridgeState): string {
  return [s.activeTabId, s.boundTabId, s.entryCount, s.segmentCount, s.activeSegment, s.archived, s.working, s.settling, s.queued].join('|')
}

export interface EntryPreview {
  type: string
  /** Tool name for a tool_use block, else ''. Without this a tool-only turn
   *  previews as empty text and reads as "nothing happened". */
  tool: string
  text: string
}

/** Flatten one entry to something assertable. Tool calls are named rather than
 *  rendered as blank, and non-rendering bookkeeping rows keep their type so a
 *  reader can see they are bookkeeping, not content. */
export function previewEntry(e: unknown): EntryPreview {
  const r = e as { type?: string; subtype?: string; message?: { content?: unknown }; content?: unknown }
  const c = r.message?.content ?? r.content
  const parts: string[] = []
  let tool = ''
  if (typeof c === 'string') parts.push(c)
  else if (Array.isArray(c)) {
    for (const b of c) {
      if (typeof b === 'string') { parts.push(b); continue }
      const blk = b as { type?: string; text?: string; name?: string; content?: unknown }
      if (blk.type === 'tool_use') { tool ||= String(blk.name ?? '?'); parts.push(`[tool:${String(blk.name ?? '?')}]`) }
      else if (blk.type === 'tool_result') parts.push('[tool_result]')
      else if (typeof blk.text === 'string') parts.push(blk.text)
    }
  }
  const type = r.subtype ? `${String(r.type ?? '?')}:${r.subtype}` : String(r.type ?? '?')
  return { type, tool, text: parts.join(' ').replace(/\s+/g, ' ').trim().slice(0, 200) }
}
