// ============================================================================
// The conversation so far, as text for an engine that joins it.
//
// A conversation can move between engines inside one chat (conversation/
// manifest.ts). A segment that starts a new native session gets the whole
// conversation as a hidden block in its first user turn — not in its system
// prompt: the session context stays exactly what a fresh chat gets (the
// provider's prompt cache stays valid), and no command-line argument carries
// the history (Linux caps one argument at 128 KiB, MAX_ARG_STRLEN). The block
// is built from Bridge's normalized transcript: user and assistant messages in
// full, tool calls and results cut like the Bridge Console shows them,
// thinking and sub-agent side chains left out.
//
// No model is called: the switch must not wait for one (owner, 2026-11-12).
// A conversation over the budget keeps its newest turns verbatim and lists
// the user's earlier requests; a rolling summary replaces that list in a
// follow-up change.
// ============================================================================

import fs from 'node:fs'
import type { ContentBlock, JsonlEntry } from '../../shared/jsonl-types'

/** Tool input/result previews, as long as the Bridge Console shows them. */
export const HANDOFF_TOOL_PREVIEW_CHARS = 400
/** History budget in UTF-8 bytes (~12-20k tokens; Cyrillic or CJK text takes
 *  2-3 bytes a character, so a character budget would not bound the size). */
export const HANDOFF_BUDGET_BYTES = 64_000
/** Share of an over-budget history kept for the newest turns, verbatim. */
const RECENT_SHARE = 0.8

export interface HandoffTurn {
  role: 'user' | 'assistant'
  text: string
}

/** Hidden context Bridge adds to a user turn. The chat does not show it, and
 *  later handoffs leave it out (it repeats what the log already holds). */
export const CONTEXT_TAG = 'bridge-context'
const CONTEXT_BLOCK = new RegExp(`\\s*<${CONTEXT_TAG}>[\\s\\S]*?</${CONTEXT_TAG}>\\s*`, 'g')

export function withoutContextBlocks(text: string): string {
  return text.includes(`<${CONTEXT_TAG}>`) ? text.replace(CONTEXT_BLOCK, '\n').trim() : text
}

/** A user message with Bridge's hidden context after it. */
export function withContextBlock(text: string, block: string): string {
  return `${text}\n\n${block}`
}

export interface HandoffSource {
  conversationId: string
  /** Engine of the segment the conversation comes from. */
  engine: 'claude' | 'codex'
  model: string | null
  /** Every engine that took part so far. */
  engines?: Array<'claude' | 'codex'>
}

export interface Handoff {
  /** Hidden block for the joining engine's first user turn. */
  text: string
  turns: number
  /** Older turns were reduced to the list of the user's requests. */
  shortened: boolean
}

function flat(text: string): string {
  return text.replace(/[\x00-\x08\x0b-\x1f\x7f\x9b]/g, '').trim()
}

function preview(text: string, limit = HANDOFF_TOOL_PREVIEW_CHARS): string {
  const t = flat(text).replace(/\s+/g, ' ')
  return t.length > limit ? `${t.slice(0, limit)}… (${t.length} chars)` : t
}

function resultText(content: string | ContentBlock[]): string {
  if (typeof content === 'string') return content
  return content
    .map((b) => (b.type === 'text' ? b.text : b.type === 'tool_result' ? resultText(b.content) : `[${b.type}]`))
    .join('\n')
}

/** User/assistant turns of a Bridge transcript, tool calls with short results.
 *  Side chains (sub-agents), CLI meta lines, thinking and Bridge's own hidden
 *  context blocks are left out. */
export function transcriptTurns(entries: readonly JsonlEntry[]): HandoffTurn[] {
  const turns: HandoffTurn[] = []
  const toolNames = new Map<string, string>()
  const push = (role: HandoffTurn['role'], text: string) => {
    const t = text.trim()
    if (!t) return
    const last = turns[turns.length - 1]
    if (last && last.role === role) last.text += `\n${t}`
    else turns.push({ role, text: t })
  }
  for (const e of entries) {
    if (e.isSidechain || e.isMeta || (e.type !== 'user' && e.type !== 'assistant')) continue
    const content = e.message?.content
    if (content === undefined) continue
    const own = (text: string) => flat(e.type === 'user' ? withoutContextBlocks(text) : text)
    if (typeof content === 'string') {
      push(e.type, own(content))
      continue
    }
    for (const block of content) {
      if (block.type === 'text') push(e.type, own(block.text))
      else if (block.type === 'tool_use') {
        if (block.id) toolNames.set(block.id, block.name)
        push('assistant', `[tool ${block.name}] ${preview(JSON.stringify(block.input ?? {}))}`)
      } else if (block.type === 'tool_result') {
        const name = toolNames.get(block.tool_use_id) ?? 'tool'
        push('assistant', `[${name} result] ${preview(resultText(block.content))}`)
      }
    }
  }
  return turns
}

export function readTranscript(file: string): JsonlEntry[] {
  let raw: string
  try {
    raw = fs.readFileSync(file, 'utf8')
  } catch {
    return []
  }
  const entries: JsonlEntry[] = []
  for (const line of raw.split('\n')) {
    if (!line.trim()) continue
    try {
      entries.push(JSON.parse(line) as JsonlEntry)
    } catch {
      /* torn line */
    }
  }
  return entries
}

const bytes = (text: string) => Buffer.byteLength(text, 'utf8')

/** The last `limit` bytes of `text`, cut on a character boundary. */
function tailBytes(text: string, limit: number): string {
  const buf = Buffer.from(text, 'utf8')
  if (buf.length <= limit) return text
  let start = buf.length - limit
  while (start < buf.length && (buf[start]! & 0xc0) === 0x80) start++
  return buf.subarray(start).toString('utf8')
}

function renderTurns(turns: HandoffTurn[]): string {
  return turns.map((t) => `${t.role === 'user' ? 'User' : 'Assistant'}: ${t.text}`).join('\n\n')
}

/** Index of the first of the newest turns that fit `budget` bytes. */
function newestWithin(turns: HandoffTurn[], budget: number): number {
  let used = 0
  let i = turns.length
  while (i > 0) {
    const size = bytes(renderTurns([turns[i - 1]!])) + 2
    if (used + size > budget && i < turns.length) break
    used += size
    i--
  }
  return i
}

function requestList(turns: HandoffTurn[], maxBytes: number): string {
  const lines = turns.filter((t) => t.role === 'user').map((t) => `- ${preview(t.text, 200)}`)
  const out: string[] = []
  let used = 0
  for (const line of lines) {
    // Room for the "… N more" line stays.
    if (used + bytes(line) + 1 > maxBytes - 24) {
      out.push(`- … ${lines.length - out.length} more`)
      break
    }
    out.push(line)
    used += bytes(line) + 1
  }
  return `Earlier in the conversation (${turns.length} turns, not included in full) the user asked:\n${out.join('\n')}`
}

const ENGINE_NAME = { claude: 'Claude', codex: 'ChatGPT' } as const

/** Fixed text of the block around the history (its bytes count too). */
const FRAME_BYTES = 900

/** The hidden block a joining engine gets with its first user turn; null for
 *  a conversation without turns. At most `budgetBytes` UTF-8 bytes. */
export function buildHandoff(
  entries: readonly JsonlEntry[],
  source: HandoffSource,
  options: { budgetBytes?: number } = {},
): Handoff | null {
  const turns = transcriptTurns(entries)
  if (turns.length === 0) return null
  const budget = Math.max(2_000, (options.budgetBytes ?? HANDOFF_BUDGET_BYTES) - FRAME_BYTES)
  const full = renderTurns(turns)
  let body = full
  let shortened = false
  if (bytes(full) > budget) {
    shortened = true
    const recentBudget = Math.floor(budget * RECENT_SHARE)
    const start = newestWithin(turns, recentBudget)
    let recent = renderTurns(turns.slice(start))
    // One newest turn can exceed the share alone (a pasted log): keep its end.
    if (bytes(recent) > recentBudget) {
      const cut = bytes(recent) - recentBudget
      recent = `[… ${cut} bytes cut …]${tailBytes(recent, recentBudget)}`
    }
    // The request list gets what the recent turns left (its header and the
    // separator take ~150 bytes); should anything still overflow, the oldest
    // text goes, never the newest turn.
    const room = Math.max(0, budget - bytes(recent) - 150)
    const earlier =
      start > 0 && room > 0
        ? `${requestList(turns.slice(0, start), room)}\n\n--- The most recent turns, verbatim ---\n\n`
        : ''
    body = tailBytes(`${earlier}${recent}`, budget)
  }
  const names = [...new Set(source.engines ?? [source.engine])].map((e) => ENGINE_NAME[e])
  const latest = `${ENGINE_NAME[source.engine]}${source.model ? ` (${source.model})` : ''}`
  const text = [
    `<${CONTEXT_TAG}>`,
    `Bridge note, not written by the user: you are joining a conversation the user is having with`,
    `${names.join(' and ')}; until now ${latest} answered. Treat the history below as your own earlier`,
    'turns: keep its facts, decisions and open tasks, and do not greet the user as if the conversation',
    'were new. Tool results are abbreviated; re-read files or re-run commands when you need their current',
    "content. Then answer the user's message above.",
    '<conversation-history>',
    body,
    '</conversation-history>',
    `</${CONTEXT_TAG}>`,
  ].join('\n')
  return { text, turns: turns.length, shortened }
}
