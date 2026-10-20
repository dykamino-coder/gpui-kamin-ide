// Server-side MCP tools that let a CLI session read the other conversations of
// its own token and project (INC-2026-0072). Unlike MCP_TOOLS, these run inside
// the Bridge server — the transcripts live here, not on the client machine — and
// they are NOT CLI-native replacements, so they stay out of MCP_TOOLS and out of
// the --disallowedTools list derived from it.

import { ArgError } from './args'
import { listSessions, listSubagents } from './list'
import { briefSession, outlineSession } from './outline'
import { readConversation, readRaw } from './read'
import { ScopeError, type CallerContext } from './scope'
import { searchSessions } from './search'
import type { ToolOutput } from './types'

const ID = { type: 'string', description: 'conversationId from project_sessions_list.' }
const AGENT = {
  type: 'string',
  description: 'Optional subagent/teammate id from project_session_subagents; reads that transcript instead.',
}
const KINDS_SCHEMA = {
  type: 'array',
  items: {
    type: 'string',
    enum: [
      'user',
      'assistant',
      'tool_result',
      'compact_summary',
      'compact_boundary',
      'system',
      'attachment',
      'meta',
      'other',
    ],
  },
  description:
    'Entry kinds to include. user = typed prompts; assistant = model turns (text, thinking, tool calls); tool_result = tool outputs; compact_summary/compact_boundary = compaction; system, attachment, meta (hook/command rows), other (bookkeeping rows).',
}
const SINCE = { type: 'string', description: 'Only entries at or after this ISO-8601 time.' }
const UNTIL = { type: 'string', description: 'Only entries at or before this ISO-8601 time.' }
const CURSOR = {
  type: 'string',
  description: 'next_cursor from the previous page, unchanged. Other paging arguments are ignored with it.',
}
const MAX_CHARS = { type: 'number', description: 'Output cap per call in characters (default 60000, max 200000).' }
const SCOPE_NOTE =
  'Read-only. Scope: only conversations of your own Bridge token AND your current project (the same client folder); other users and other projects are invisible. Credentials in the text are replaced by [REDACTED:kind].'

export const PROJECT_SESSION_TOOLS = [
  {
    name: 'project_sessions_list',
    description: `Lists the Claude conversations of this project on the Bridge server, newest activity first: conversation_id, title (custom title or first prompt), started_at, last_activity_at, size_bytes, lines, subagents count, source (live or archive for expired transcripts), live (a CLI session is running it), attached (a client is connected) and self (it is THIS conversation). Use it to find a neighbouring chat or an earlier conversation whose history you need to recover, then read it with project_session_brief / project_session_read. ${SCOPE_NOTE}`,
    inputSchema: {
      type: 'object',
      properties: {
        limit: { type: 'number', description: 'Conversations per page (default 50, max 200).' },
        offset: { type: 'number', description: 'next_offset from the previous page.' },
        include_archived: {
          type: 'boolean',
          description: 'Include transcripts kept only in the Bridge archive (default true).',
        },
      },
    },
  },
  {
    name: 'project_session_read',
    description: `Reads one conversation of this project as readable text, paged. Each entry starts with "#<index> <timestamp> <kind>"; index is the JSONL line number used by all project_session_* tools. detail "text" (default) shows prompts, assistant text, compact summaries and one-line tool calls; include_tools adds full tool arguments and tool results, include_thinking adds thinking. detail "full" shows everything: thinking, tool calls with arguments, full tool results, attachments, system rows, model and token usage, other rows as JSON. Filter with kinds, since/until, from_index/to_index, or tail (last N matching entries). The last block is page JSON: next_cursor (null when done), first/last index, redactions. ${SCOPE_NOTE}`,
    inputSchema: {
      type: 'object',
      properties: {
        conversation_id: ID,
        agent_id: AGENT,
        detail: { type: 'string', enum: ['text', 'full'], description: 'text (default) or full.' },
        include_tools: {
          type: 'boolean',
          description: 'Show tool arguments and tool results (default: false for text, true for full).',
        },
        include_thinking: {
          type: 'boolean',
          description: 'Show thinking blocks (default: false for text, true for full).',
        },
        kinds: KINDS_SCHEMA,
        since: SINCE,
        until: UNTIL,
        from_index: { type: 'number', description: 'Start at this entry index (inclusive).' },
        to_index: { type: 'number', description: 'Stop before this entry index (exclusive).' },
        tail: { type: 'number', description: 'Return only the last N matching entries (1..1000).' },
        cursor: CURSOR,
        max_chars: MAX_CHARS,
      },
      required: ['conversation_id'],
    },
  },
  {
    name: 'project_session_raw',
    description: `Returns the exact JSONL lines of one conversation of this project for exact recovery, one line per entry, paged by from_index/to_index, tail (last N lines) or cursor. Lines larger than a page are split; concatenate pages until partial_line is false. Only credential redaction alters the bytes (reported as redactions). The last block is page JSON with next_cursor. ${SCOPE_NOTE}`,
    inputSchema: {
      type: 'object',
      properties: {
        conversation_id: ID,
        agent_id: AGENT,
        from_index: { type: 'number', description: 'First line index (inclusive).' },
        to_index: { type: 'number', description: 'Stop before this line index (exclusive).' },
        tail: { type: 'number', description: 'Only the last N lines (1..1000).' },
        cursor: CURSOR,
        max_chars: MAX_CHARS,
      },
      required: ['conversation_id'],
    },
  },
  {
    name: 'project_session_search',
    description: `Full-text search across ALL conversations of this project (optionally their subagents): returns conversation_id, entry index, timestamp, kind, match count and snippets, newest conversations first, paged with next_cursor. Searches prompts, assistant text, thinking, tool arguments and tool results. Literal by default (case-insensitive); regex: true for JavaScript regex (no backreferences or nested repeats). Narrow with kinds (role), since/until and conversation_ids. Then open a hit with project_session_read from_index=<index>. ${SCOPE_NOTE}`,
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Text or regex to find.' },
        regex: { type: 'boolean', description: 'Treat query as a regex (default false).' },
        case_sensitive: { type: 'boolean', description: 'Default false.' },
        kinds: KINDS_SCHEMA,
        since: SINCE,
        until: UNTIL,
        conversation_ids: { type: 'array', items: { type: 'string' }, description: 'Restrict to these conversations.' },
        include_subagents: {
          type: 'boolean',
          description: 'Also search subagent/teammate transcripts (default false).',
        },
        max_results: { type: 'number', description: 'Matching entries per page (default 30, max 200).' },
        context_chars: { type: 'number', description: 'Snippet context on each side (default 120).' },
        cursor: CURSOR,
        max_chars: MAX_CHARS,
      },
      required: ['query'],
    },
  },
  {
    name: 'project_session_outline',
    description: `Statistics and outline of one conversation of this project: entry counts by kind and row type, compact segments (boundary indices, trigger, pre-compact tokens, summaries), tools used with counts, tool errors, files touched by tool calls, models, token usage, git branches, time range and the first/last prompts with their indices. Use it to decide which index range to read. ${SCOPE_NOTE}`,
    inputSchema: {
      type: 'object',
      properties: { conversation_id: ID, agent_id: AGENT },
      required: ['conversation_id'],
    },
  },
  {
    name: 'project_session_subagents',
    description: `Lists the subagent and teammate transcripts of one conversation of this project: agent_id, agent_type, description, first prompt, times, size and lines. Read one by passing its agent_id to project_session_read, _raw, _outline or _brief. ${SCOPE_NOTE}`,
    inputSchema: {
      type: 'object',
      properties: {
        conversation_id: ID,
        limit: { type: 'number', description: 'Agents per page (default 100, max 500).' },
        offset: { type: 'number', description: 'next_offset from the previous page.' },
      },
      required: ['conversation_id'],
    },
  },
  {
    name: 'project_session_brief',
    description: `Builds a condensed Markdown continuation brief of one conversation of this project: title, time range, the latest compact summary, recently touched files and the last N turns (each user prompt with the final assistant text and the tools it used). Paste it into, or use it to seed, a new chat that continues the work. It only reads; no session is modified. ${SCOPE_NOTE}`,
    inputSchema: {
      type: 'object',
      properties: {
        conversation_id: ID,
        agent_id: AGENT,
        turns: { type: 'number', description: 'Recent turns to include (default 10, max 50).' },
        max_chars: { type: 'number', description: 'Brief size cap (default 40000, max 200000).' },
      },
      required: ['conversation_id'],
    },
  },
] as const

export type ProjectSessionToolName = (typeof PROJECT_SESSION_TOOLS)[number]['name']

const HANDLERS: Record<
  ProjectSessionToolName,
  (ctx: CallerContext, args: Record<string, unknown>) => Promise<ToolOutput>
> = {
  project_sessions_list: listSessions,
  project_session_read: readConversation,
  project_session_raw: readRaw,
  project_session_search: searchSessions,
  project_session_outline: outlineSession,
  project_session_subagents: listSubagents,
  project_session_brief: briefSession,
}

export function projectSessionToolsEnabled(): boolean {
  return (process.env.KAMIN_PROJECT_SESSION_TOOLS ?? '').toLowerCase() !== 'off'
}

export function isProjectSessionTool(name: string): name is ProjectSessionToolName {
  return Object.prototype.hasOwnProperty.call(HANDLERS, name)
}

export type Outcome = 'ok' | 'not_found' | 'invalid' | 'error'

export interface AccessRecord {
  tool: ProjectSessionToolName
  outcome: Outcome
  conversationId?: string
  self?: boolean
  bytes: number
  entries?: number
  /** Unexpected failures only (filesystem errors): for the Bridge log, never the journal. */
  error?: string
}

export interface McpToolResult {
  content: Array<{ type: 'text'; text: string }>
  isError?: boolean
}

/** Run one tool; never throws. `log` receives metadata only — no content. */
export async function runProjectSessionTool(
  name: ProjectSessionToolName,
  args: Record<string, unknown>,
  ctx: CallerContext,
  log: (record: AccessRecord) => void,
): Promise<McpToolResult> {
  const requested = typeof args.conversation_id === 'string' ? args.conversation_id : undefined
  try {
    const out = await HANDLERS[name](ctx, args ?? {})
    const content = out.blocks.map((text) => ({ type: 'text' as const, text }))
    log({
      tool: name,
      outcome: 'ok',
      conversationId: out.conversationId,
      self: out.self,
      bytes: content.reduce((n, b) => n + Buffer.byteLength(b.text), 0),
      entries: out.entries,
    })
    return { content }
  } catch (err) {
    const outcome: Outcome = err instanceof ScopeError ? err.outcome : err instanceof ArgError ? 'invalid' : 'error'
    const message = err instanceof ScopeError || err instanceof ArgError ? err.message : 'project session tool failed'
    // The requested id is logged only as a pseudonymous ref by the caller's logger.
    const record: AccessRecord = { tool: name, outcome, conversationId: requested, bytes: 0 }
    if (outcome === 'error') record.error = err instanceof Error ? err.message : String(err)
    log(record)
    return { content: [{ type: 'text', text: `Error: ${message}` }], isError: true }
  }
}
