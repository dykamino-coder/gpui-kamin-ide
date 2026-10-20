// project_session_outline and project_session_brief (INC-2026-0072).
// Both stream the whole transcript once and keep only bounded aggregates:
// capped maps, a ring of recent turns and the latest compact summary.

import { intArg } from './args'
import { openTranscript, readLines } from './line-reader'
import { redact } from './redact'
import { assistantText, classify, parseEntry, promptText, timestampOf, type Entry, type Kind } from './render'
import type { CallerContext } from './scope'
import { resolveTarget } from './target'
import type { ToolOutput } from './types'

const MAX_TOOLS = 200
const MAX_FILES = 500
const PROMPT_CHARS = 300

type Block = Record<string, unknown>

function oneLine(s: string, max: number): string {
  const flat = s.replace(/\s+/g, ' ').trim()
  return flat.length > max ? flat.slice(0, max) + '…' : flat
}

function toolUses(entry: Entry): Block[] {
  const content = (entry.message as { content?: unknown } | undefined)?.content
  return Array.isArray(content) ? (content as Block[]).filter((b) => b?.type === 'tool_use') : []
}

/** Paths named by a tool call's arguments (Read/Write/Edit/NotebookEdit/Glob/Grep...). */
function touchedPaths(b: Block): string[] {
  const input = b.input as Record<string, unknown> | undefined
  if (!input || typeof input !== 'object') return []
  return ['file_path', 'notebook_path', 'path']
    .map((k) => input[k])
    .filter((v): v is string => typeof v === 'string' && v.length > 0 && v.length < 1024)
}

function bump(map: Map<string, number>, key: string, cap: number): void {
  const prev = map.get(key)
  if (prev !== undefined) {
    // Re-insert so iteration order is "most recently touched last".
    map.delete(key)
    map.set(key, prev + 1)
  } else if (map.size < cap) map.set(key, 1)
}

export async function outlineSession(ctx: CallerContext, args: Record<string, unknown>): Promise<ToolOutput> {
  const target = await resolveTarget(ctx, args)
  const kinds: Record<string, number> = {}
  const types: Record<string, number> = {}
  const tools = new Map<string, number>()
  const files = new Map<string, number>()
  const models = new Map<string, number>()
  const branches = new Set<string>()
  const compacts: Array<Record<string, unknown>> = []
  const firstPrompts: Array<{ index: number; text: string }> = []
  const lastPrompts: Array<{ index: number; text: string }> = []
  const usage = { input: 0, output: 0, cache_read: 0, cache_write: 0, assistant_messages: 0 }
  let lastMsgId: string | null = null
  let lines = 0
  let oversized = 0
  let unparsable = 0
  let toolErrors = 0
  let title: string | null = null
  let first: string | null = null
  let last: string | null = null
  const fh = await openTranscript(target.file)
  try {
    for await (const line of readLines(fh)) {
      lines++
      if (line.text === null) {
        oversized++
        continue
      }
      const entry = parseEntry(line.text)
      if (!entry) {
        if (line.text.trim()) unparsable++
        continue
      }
      const kind: Kind = classify(entry)
      kinds[kind] = (kinds[kind] ?? 0) + 1
      const type = typeof entry.type === 'string' ? entry.type : '(none)'
      if (Object.keys(types).length < 100 || type in types) types[type] = (types[type] ?? 0) + 1
      const ts = timestampOf(entry)
      if (ts) {
        first ??= ts
        last = ts
      }
      if (typeof entry.gitBranch === 'string' && branches.size < 20) branches.add(entry.gitBranch)
      if (entry.type === 'custom-title' && typeof entry.customTitle === 'string') title = entry.customTitle
      if (kind === 'user') {
        const text = oneLine(promptText(entry), PROMPT_CHARS)
        if (text) {
          if (firstPrompts.length < 3) firstPrompts.push({ index: line.index, text })
          lastPrompts.push({ index: line.index, text })
          if (lastPrompts.length > 3) lastPrompts.shift()
        }
      } else if (kind === 'compact_boundary') {
        const meta = entry.compactMetadata as { trigger?: unknown; preTokens?: unknown } | undefined
        compacts.push({
          index: line.index,
          at: ts || null,
          trigger: meta?.trigger ?? null,
          pre_tokens: meta?.preTokens ?? null,
        })
      } else if (kind === 'compact_summary') {
        compacts.push({ index: line.index, at: ts || null, summary_chars: promptText(entry).length })
      } else if (kind === 'tool_result') {
        const content = (entry.message as { content?: unknown }).content as Block[]
        toolErrors += content.filter((b) => b?.type === 'tool_result' && b.is_error === true).length
      } else if (kind === 'assistant') {
        for (const b of toolUses(entry)) {
          if (typeof b.name === 'string') bump(tools, b.name, MAX_TOOLS)
          for (const p of touchedPaths(b)) bump(files, p, MAX_FILES)
        }
        const msg = entry.message as { id?: unknown; model?: unknown; usage?: Record<string, number> }
        const id = typeof msg.id === 'string' ? msg.id : null
        if (id === null || id !== lastMsgId) {
          lastMsgId = id
          usage.assistant_messages++
          if (typeof msg.model === 'string') bump(models, msg.model, 50)
          const u = msg.usage
          if (u) {
            usage.input += u.input_tokens ?? 0
            usage.output += u.output_tokens ?? 0
            usage.cache_read += u.cache_read_input_tokens ?? 0
            usage.cache_write += u.cache_creation_input_tokens ?? 0
          }
        }
      }
    }
  } finally {
    await fh.close()
  }
  const sorted = (m: Map<string, number>) => Object.fromEntries([...m].sort((a, b) => b[1] - a[1]))
  const body = JSON.stringify(
    {
      conversation_id: target.ref.conversationId,
      agent_id: target.agentId,
      source: target.ref.source,
      title: title ? oneLine(title, 120) : (firstPrompts[0]?.text.slice(0, 120) ?? null),
      first_entry_at: first,
      last_entry_at: last,
      lines,
      oversized_lines: oversized,
      unparsable_lines: unparsable,
      kinds,
      types,
      compact_segments: compacts,
      tools: sorted(tools),
      tool_errors: toolErrors,
      files_touched: sorted(files),
      files_capped: files.size >= MAX_FILES,
      models: sorted(models),
      usage,
      git_branches: [...branches],
      first_prompts: firstPrompts,
      last_prompts: lastPrompts,
    },
    null,
    1,
  )
  const r = redact(body)
  return { blocks: [r.text], conversationId: target.ref.conversationId, self: target.self, entries: lines }
}

interface Turn {
  index: number
  at: string
  prompt: string
  answer: string
  tools: Map<string, number>
}

const TURN_PROMPT_CHARS = 4000
const TURN_ANSWER_CHARS = 6000
const SUMMARY_KEEP_CHARS = 120_000

export async function briefSession(ctx: CallerContext, args: Record<string, unknown>): Promise<ToolOutput> {
  const turnsWanted = intArg(args.turns, 'turns', 10, 1, 50)
  const maxChars = intArg(args.max_chars, 'max_chars', 40_000, 2000, 200_000)
  const target = await resolveTarget(ctx, args)
  const ring: Turn[] = []
  const files = new Map<string, number>()
  let summary: { index: number; text: string } | null = null
  let title: string | null = null
  let first: string | null = null
  let last: string | null = null
  const fh = await openTranscript(target.file)
  try {
    for await (const line of readLines(fh)) {
      const entry = parseEntry(line.text)
      if (!entry) continue
      const kind = classify(entry)
      const ts = timestampOf(entry)
      if (ts) {
        first ??= ts
        last = ts
      }
      if (entry.type === 'custom-title' && typeof entry.customTitle === 'string') title = entry.customTitle
      if (kind === 'compact_summary') {
        summary = { index: line.index, text: promptText(entry).slice(0, SUMMARY_KEEP_CHARS) }
      } else if (kind === 'user') {
        const prompt = promptText(entry).trim()
        if (!prompt) continue
        ring.push({
          index: line.index,
          at: ts,
          prompt: prompt.slice(0, TURN_PROMPT_CHARS),
          answer: '',
          tools: new Map(),
        })
        if (ring.length > turnsWanted) ring.shift()
      } else if (kind === 'assistant') {
        const turn = ring[ring.length - 1]
        for (const b of toolUses(entry)) {
          if (turn && typeof b.name === 'string') bump(turn.tools, b.name, 50)
          for (const p of touchedPaths(b)) bump(files, p, MAX_FILES)
        }
        const text = assistantText(entry).trim()
        // Keep the END of the answer: the final assistant text is the conclusion.
        if (turn && text) turn.answer = (turn.answer ? `${turn.answer}\n\n${text}` : text).slice(-TURN_ANSWER_CHARS)
      }
    }
  } finally {
    await fh.close()
  }

  const head = [
    `# Continuation brief: ${oneLine(title ?? ring[0]?.prompt ?? target.ref.conversationId, 120)}`,
    '',
    `Conversation \`${target.ref.conversationId}\`${target.agentId ? ` (subagent \`${target.agentId}\`)` : ''}, ${first ?? '?'} to ${last ?? '?'}, source ${target.ref.source}.`,
    'Produced read-only by project_session_brief; nothing was modified. Use project_session_read for full detail.',
    '',
  ].join('\n')
  const recentFiles = [...files].slice(-30).reverse()
  const filesPart = recentFiles.length
    ? `## Recently touched files\n${recentFiles.map(([p, n]) => `- ${p} (${n})`).join('\n')}\n\n`
    : ''
  let summaryPart = ''
  if (summary) {
    const room = Math.floor(maxChars * 0.4)
    const text = summary.text.length > room ? summary.text.slice(0, room) + '\n…[summary truncated]' : summary.text
    summaryPart = `## Latest compact summary (entry #${summary.index})\n${text}\n\n`
  }
  let remaining = maxChars - head.length - filesPart.length - summaryPart.length - 200
  const turnParts: string[] = []
  for (let i = ring.length - 1; i >= 0 && remaining > 0; i--) {
    const t = ring[i]!
    const tools = [...t.tools].map(([n, c]) => `${n}×${c}`).join(', ')
    let part = `### Turn #${t.index}${t.at ? ` · ${t.at}` : ''}\n**User:** ${t.prompt}\n\n**Assistant:** ${t.answer || '(no text)'}\n${tools ? `_Tools: ${tools}_\n` : ''}\n`
    if (part.length > remaining) part = part.slice(0, remaining) + '\n…[turn truncated]\n'
    turnParts.unshift(part)
    remaining -= part.length
  }
  const turnsPart = `## Last ${turnParts.length} turn${turnParts.length === 1 ? '' : 's'}\n\n${turnParts.join('')}`
  const r = redact(head + summaryPart + filesPart + turnsPart)
  return {
    blocks: [r.text.slice(0, maxChars)],
    conversationId: target.ref.conversationId,
    self: target.self,
    entries: turnParts.length,
  }
}
