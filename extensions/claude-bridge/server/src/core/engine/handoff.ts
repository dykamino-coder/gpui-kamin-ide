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

/** The turn pieces one transcript entry contributes, in order. Side chains
 *  (sub-agents), CLI meta lines, thinking and Bridge's own hidden context
 *  blocks are left out; tool calls and results are previews. */
function entryPieces(e: JsonlEntry, toolNames: Map<string, string>): Array<[HandoffTurn['role'], string]> {
  if (e.isSidechain || e.isMeta || (e.type !== 'user' && e.type !== 'assistant')) return []
  const content = e.message?.content
  if (content === undefined) return []
  const own = (text: string) => flat(e.type === 'user' ? withoutContextBlocks(text) : text)
  if (typeof content === 'string') return [[e.type, own(content)]]
  const out: Array<[HandoffTurn['role'], string]> = []
  for (const block of content) {
    if (block.type === 'text') out.push([e.type, own(block.text)])
    else if (block.type === 'tool_use') {
      if (block.id) toolNames.set(block.id, block.name)
      out.push(['assistant', `[tool ${block.name}] ${preview(JSON.stringify(block.input ?? {}))}`])
    } else if (block.type === 'tool_result') {
      const name = toolNames.get(block.tool_use_id) ?? 'tool'
      out.push(['assistant', `[${name} result] ${preview(resultText(block.content))}`])
    }
  }
  return out
}

/** User/assistant turns of a Bridge transcript, tool calls with short results.
 *  Consecutive pieces of one role form one turn. */
export function transcriptTurns(entries: readonly JsonlEntry[]): HandoffTurn[] {
  const turns: HandoffTurn[] = []
  const toolNames = new Map<string, string>()
  for (const e of entries) {
    for (const [role, text] of entryPieces(e, toolNames)) {
      const t = text.trim()
      if (!t) continue
      const last = turns[turns.length - 1]
      if (last && last.role === role) last.text += `\n${t}`
      else turns.push({ role, text: t })
    }
  }
  return turns
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

const label = (role: HandoffTurn['role']) => (role === 'user' ? 'User' : 'Assistant')

function renderTurns(turns: readonly HandoffTurn[]): string {
  return turns.map((t) => `${label(t.role)}: ${t.text}`).join('\n\n')
}

/** Rendered size of one turn plus its separator, as the budget counts it. */
const turnBytes = (t: HandoffTurn) => bytes(`${label(t.role)}: ${t.text}`) + 2

const ENGINE_NAME = { claude: 'Claude', codex: 'ChatGPT' } as const

/** Fixed text of the block around the history (its bytes count too). */
const FRAME_BYTES = 900

function historyBudget(options: { budgetBytes?: number }): number {
  return Math.max(2_000, (options.budgetBytes ?? HANDOFF_BUDGET_BYTES) - FRAME_BYTES)
}

interface KeptTurn extends HandoffTurn {
  index: number
  /** turnBytes once the turn is complete; null while it may still grow. */
  size: number | null
  /** turnBytes so far, kept up to date while the turn grows. */
  running: number
}

/**
 * Everything the block needs from a conversation of any length, collected one
 * entry at a time: the newest turns (as many as can appear verbatim, plus the
 * one before them), the earlier user requests as far as their list can reach,
 * and counts. Memory stays near the budget whatever the transcript size; the
 * result is the one `transcriptTurns` + the whole-list build would give.
 */
class TurnDigest {
  total = 0
  users = 0
  /** Sum of turnBytes of completed turns. */
  private doneBytes = 0
  /** Sum of sizes of the completed turns in `kept`. */
  private keptDone = 0
  readonly kept: KeptTurn[] = []
  /** "- <request>" lines of the first user turns, while their list can use them. */
  readonly requests: Array<{ index: number; line: string }> = []
  private requestBytes = 0
  private readonly toolNames = new Map<string, string>()

  constructor(private readonly budget: number) {}

  add(e: JsonlEntry): void {
    for (const [role, text] of entryPieces(e, this.toolNames)) {
      const t = text.trim()
      if (!t) continue
      const last = this.kept[this.kept.length - 1]
      if (last && last.size === null && last.role === role) {
        last.text += `\n${t}`
        last.running += bytes(t) + 1
        continue
      }
      if (last) this.complete(last)
      this.kept.push({ role, text: t, index: this.total++, size: null, running: turnBytes({ role, text: t }) })
      if (role === 'user') this.users++
      this.trim()
    }
  }

  private complete(t: KeptTurn): void {
    if (t.size !== null) return
    t.size = t.running
    this.doneBytes += t.size
    this.keptDone += t.size
    if (t.role === 'user' && this.requestBytes <= this.budget) {
      const line = `- ${preview(t.text, 200)}`
      this.requests.push({ index: t.index, line })
      this.requestBytes += bytes(line) + 1
    }
  }

  /** Drop the oldest kept turn while the newer ones alone exceed the budget:
   *  they are all the verbatim part can hold, and the dropped turn could at
   *  most have been the one before them, which is kept. */
  private trim(): void {
    const open = this.kept[this.kept.length - 1]
    const growing = open && open.size === null ? open.running : 0
    while (this.kept.length > 1 && this.keptDone + growing - this.kept[0]!.size! > this.budget) {
      this.keptDone -= this.kept.shift()!.size!
    }
  }

  finish(): this {
    const last = this.kept[this.kept.length - 1]
    if (last) this.complete(last)
    this.trim()
    return this
  }

  /** Rendered size of the whole conversation (renderTurns of every turn). */
  fullBytes(): number {
    return this.total === 0 ? 0 : this.doneBytes - 2
  }
}

function requestList(d: TurnDigest, start: number, maxBytes: number): string {
  const before = d.kept.filter((t) => t.index >= start && t.role === 'user').length
  const userCount = d.users - before
  const out: string[] = []
  let used = 0
  for (const { index, line } of d.requests) {
    if (index >= start || out.length === userCount) break
    // Room for the "… N more" line stays.
    if (used + bytes(line) + 1 > maxBytes - 24) break
    out.push(line)
    used += bytes(line) + 1
  }
  if (out.length < userCount) out.push(`- … ${userCount - out.length} more`)
  return `Earlier in the conversation (${start} turns, not included in full) the user asked:\n${out.join('\n')}`
}

function fromDigest(d: TurnDigest, source: HandoffSource, budget: number): Handoff | null {
  if (d.total === 0) return null
  const kept = d.kept
  let body: string
  let shortened = false
  if (d.fullBytes() <= budget) {
    body = renderTurns(kept)
  } else {
    shortened = true
    const recentBudget = Math.floor(budget * RECENT_SHARE)
    // The newest turns that fit the share (the newest one always).
    let used = 0
    let i = kept.length
    while (i > 0) {
      const size = kept[i - 1]!.size!
      if (used + size > recentBudget && i < kept.length) break
      used += size
      i--
    }
    let start = i < kept.length ? kept[i]!.index : d.total
    let recent = renderTurns(kept.slice(i))
    // The turn before them did not fit whole (a pasted log, a long answer):
    // its end goes in with the room left — the end of a paste is usually what
    // the conversation went on about.
    const left = recentBudget - bytes(recent) - 2
    if (start > 0 && left > 1_000) {
      const prev = kept[i - 1]!
      const keep = left - 80
      const cut = bytes(prev.text) - keep
      recent = `${label(prev.role)}: [… first ${cut} bytes cut …]${tailBytes(prev.text, keep)}${recent ? `\n\n${recent}` : ''}`
      start--
    }
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
      start > 0 && room > 0 ? `${requestList(d, start, room)}\n\n--- The most recent turns, verbatim ---\n\n` : ''
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
  return { text, turns: d.total, shortened }
}

/** The hidden block a joining engine gets with its first user turn; null for
 *  a conversation without turns. At most `budgetBytes` UTF-8 bytes. */
export function buildHandoff(
  entries: readonly JsonlEntry[],
  source: HandoffSource,
  options: { budgetBytes?: number } = {},
): Handoff | null {
  const budget = historyBudget(options)
  const d = new TurnDigest(budget)
  for (const e of entries) d.add(e)
  return fromDigest(d.finish(), source, budget)
}

/** Lines of a transcript handed to the digest per event-loop turn. */
const READ_CHUNK_BYTES = 1024 * 1024

/**
 * `buildHandoff` of a transcript file, read in chunks with the event loop
 * free between them: a long conversation is neither held in memory nor parsed
 * in one blocking pass (a shared server answers every other user meanwhile).
 * A missing file is an empty conversation; a torn line is skipped.
 */
export async function readHandoff(
  file: string,
  source: HandoffSource,
  options: { budgetBytes?: number } = {},
): Promise<Handoff | null> {
  const budget = historyBudget(options)
  const d = new TurnDigest(budget)
  let fh: fs.promises.FileHandle
  try {
    fh = await fs.promises.open(file, 'r')
  } catch {
    return fromDigest(d.finish(), source, budget)
  }
  try {
    const buf = Buffer.allocUnsafe(READ_CHUNK_BYTES)
    let pos = 0
    let carry: Buffer[] = []
    const take = (line: Buffer) => {
      const text = line.toString('utf8')
      if (!text.trim()) return
      try {
        d.add(JSON.parse(text) as JsonlEntry)
      } catch {
        /* torn line */
      }
    }
    for (;;) {
      const { bytesRead } = await fh.read(buf, 0, READ_CHUNK_BYTES, pos)
      if (bytesRead === 0) break
      pos += bytesRead
      let from = 0
      for (let nl = buf.indexOf(10, 0); nl !== -1 && nl < bytesRead; nl = buf.indexOf(10, from)) {
        const piece = buf.subarray(from, nl)
        take(carry.length ? Buffer.concat([...carry, piece]) : piece)
        carry = []
        from = nl + 1
      }
      if (from < bytesRead) carry.push(Buffer.from(buf.subarray(from, bytesRead)))
      // Yield between chunks.
      await new Promise<void>((resolve) => setImmediate(resolve))
    }
    if (carry.length) take(Buffer.concat(carry))
  } finally {
    await fh.close()
  }
  return fromDigest(d.finish(), source, budget)
}
