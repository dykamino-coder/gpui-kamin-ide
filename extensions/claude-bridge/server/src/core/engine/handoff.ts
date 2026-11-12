// ============================================================================
// The conversation so far, as text for an engine that joins it.
//
// A conversation can move between engines inside one chat (conversation/
// manifest.ts). A segment that starts a new native session gets the whole
// conversation in its session context (Claude: --append-system-prompt; codex:
// developerInstructions), built here from Bridge's normalized transcript: user
// and assistant messages in full, tool calls and results cut like the Bridge
// Console shows them, thinking and sub-agent side chains left out.
//
// No model is called: the switch must not wait for one (owner, 2026-11-12).
// A conversation longer than the budget keeps its newest turns verbatim and
// lists the user's earlier requests; a ready-made rolling summary replaces
// that list in a follow-up change.
// ============================================================================

import fs from 'node:fs'
import type { ContentBlock, JsonlEntry } from '../../shared/jsonl-types'

/** Tool input/result previews, as long as the Bridge Console shows them. */
export const HANDOFF_TOOL_PREVIEW_CHARS = 400
/** History budget: ~12-16k tokens. Claude gets it in one CLI argument, which
 *  Linux caps at 128 KiB: 48k characters stay under that even when every one
 *  is two UTF-8 bytes (Cyrillic), with room for the rest of the prompt. */
export const HANDOFF_BUDGET_CHARS = 48_000
/** Share of an over-budget history kept for the newest turns, verbatim. */
const RECENT_SHARE = 0.8

export interface HandoffTurn {
  role: 'user' | 'assistant'
  text: string
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
  /** Section for the new engine's session context. */
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
 *  Side chains (sub-agents), CLI meta lines and thinking are left out. */
export function transcriptTurns(entries: JsonlEntry[]): HandoffTurn[] {
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
    if (typeof content === 'string') {
      push(e.type, flat(content))
      continue
    }
    for (const block of content) {
      if (block.type === 'text') push(e.type, flat(block.text))
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

function renderTurns(turns: HandoffTurn[]): string {
  return turns.map((t) => `${t.role === 'user' ? 'User' : 'Assistant'}: ${t.text}`).join('\n\n')
}

/** Index of the first of the newest turns that fit `budget` characters. */
function newestWithin(turns: HandoffTurn[], budget: number): number {
  let used = 0
  let i = turns.length
  while (i > 0) {
    const size = renderTurns([turns[i - 1]!]).length + 2
    if (used + size > budget && i < turns.length) break
    used += size
    i--
  }
  return i
}

function requestList(turns: HandoffTurn[], maxChars: number): string {
  const lines = turns.filter((t) => t.role === 'user').map((t) => `- ${preview(t.text, 200)}`)
  const out: string[] = []
  let used = 0
  for (const line of lines) {
    if (used + line.length + 1 > maxChars) {
      out.push(`- … ${lines.length - out.length} more`)
      break
    }
    out.push(line)
    used += line.length + 1
  }
  return `Earlier in the conversation (${turns.length} turns, not included in full) the user asked:\n${out.join('\n')}`
}

const ENGINE_NAME = { claude: 'Claude', codex: 'ChatGPT' } as const

/** The history section a joining engine starts with; null for no turns. */
export function buildHandoff(
  entries: JsonlEntry[],
  source: HandoffSource,
  options: { budgetChars?: number } = {},
): Handoff | null {
  const turns = transcriptTurns(entries)
  if (turns.length === 0) return null
  const budget = options.budgetChars ?? HANDOFF_BUDGET_CHARS
  const full = renderTurns(turns)
  let body = full
  let shortened = false
  if (full.length > budget) {
    shortened = true
    const recentBudget = Math.floor(budget * RECENT_SHARE)
    const start = newestWithin(turns, recentBudget)
    let recent = renderTurns(turns.slice(start))
    // One newest turn can exceed the share alone (a pasted log): keep its end.
    if (recent.length > recentBudget) {
      recent = `[… ${recent.length - recentBudget} characters cut …]${recent.slice(-recentBudget)}`
    }
    const room = Math.max(1_000, budget - recent.length - 200)
    const earlier =
      start > 0 ? `${requestList(turns.slice(0, start), room)}\n\n--- The most recent turns, verbatim ---\n\n` : ''
    body = `${earlier}${recent}`
  }
  const names = [...new Set(source.engines ?? [source.engine])].map((e) => ENGINE_NAME[e])
  const latest = `${ENGINE_NAME[source.engine]}${source.model ? ` (${source.model})` : ''}`
  const text = [
    '## Conversation so far',
    '',
    `You are joining a conversation the user is having in Bridge with ${names.join(' and ')}; until now ${latest} answered.`,
    'Treat the history below as your own earlier turns: keep its facts, decisions and open tasks, and do not',
    'greet the user as if the conversation were new. Tool results are abbreviated; re-read files or re-run',
    'commands when you need their current content.',
    '',
    '<conversation-history>',
    body,
    '</conversation-history>',
  ].join('\n')
  return { text, turns: turns.length, shortened }
}
