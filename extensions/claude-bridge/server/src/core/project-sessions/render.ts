// Readable rendering of Claude CLI transcript entries (INC-2026-0072).
//
// The JSONL is the CLI's private format and carries many row types; this module
// is the single place that classifies a row into a stable "kind" the tools filter
// by, and renders it at two levels: `text` (the conversation as a person reads
// it, tools summarised) and `full` (every block: thinking, tool arguments, tool
// results, attachments, system rows and any other row as JSON).

export const KINDS = [
  'user',
  'assistant',
  'tool_result',
  'compact_summary',
  'compact_boundary',
  'system',
  'attachment',
  'meta',
  'other',
] as const
export type Kind = (typeof KINDS)[number]

export interface RenderOptions {
  detail: 'text' | 'full'
  includeTools: boolean
  includeThinking: boolean
}

type Block = Record<string, unknown>
export type Entry = Record<string, unknown>

const str = (v: unknown): string => (typeof v === 'string' ? v : '')

function blocks(entry: Entry): Block[] | null {
  const content = (entry.message as { content?: unknown } | undefined)?.content
  return Array.isArray(content) ? (content.filter((b) => b && typeof b === 'object') as Block[]) : null
}

export function classify(entry: Entry): Kind {
  const type = entry.type
  if (type === 'user') {
    if (entry.isCompactSummary === true) return 'compact_summary'
    const bs = blocks(entry)
    if (bs && bs.some((b) => b.type === 'tool_result')) return 'tool_result'
    if (entry.isMeta === true) return 'meta'
    return 'user'
  }
  if (type === 'assistant') return 'assistant'
  if (type === 'system') return entry.subtype === 'compact_boundary' ? 'compact_boundary' : 'system'
  if (type === 'summary') return 'compact_summary'
  if (type === 'attachment') return 'attachment'
  return 'other'
}

export function defaultKinds(opts: RenderOptions): Set<Kind> {
  if (opts.detail === 'full') return new Set(KINDS)
  const kinds = new Set<Kind>(['user', 'assistant', 'compact_summary', 'compact_boundary'])
  if (opts.includeTools) kinds.add('tool_result')
  return kinds
}

export function timestampOf(entry: Entry): string {
  return str(entry.timestamp)
}

/** Text of a user prompt or compact summary (string or text blocks). */
export function promptText(entry: Entry): string {
  const content = (entry.message as { content?: unknown } | undefined)?.content
  if (typeof content === 'string') return content
  if (entry.type === 'summary') return str(entry.summary)
  const bs = blocks(entry)
  return bs
    ? bs
        .filter((b) => b.type === 'text')
        .map((b) => str(b.text))
        .join('\n')
    : ''
}

/** Assistant text blocks only (what the user saw as the answer). */
export function assistantText(entry: Entry): string {
  const bs = blocks(entry)
  return bs
    ? bs
        .filter((b) => b.type === 'text')
        .map((b) => str(b.text))
        .join('\n')
    : ''
}

function oneLine(s: string, max: number): string {
  const flat = s.replace(/\s+/g, ' ').trim()
  return flat.length > max ? flat.slice(0, max) + '…' : flat
}

function toolResultText(content: unknown): string {
  if (typeof content === 'string') return content
  if (!Array.isArray(content)) return content === undefined ? '' : JSON.stringify(content)
  return content
    .map((b: Block) => (b?.type === 'text' ? str(b.text) : b?.type === 'image' ? '[image]' : JSON.stringify(b)))
    .join('\n')
}

function usageLine(entry: Entry): string {
  const msg = entry.message as { model?: unknown; usage?: Record<string, unknown> } | undefined
  const parts: string[] = []
  if (typeof msg?.model === 'string') parts.push(`model=${msg.model}`)
  const u = msg?.usage
  if (u && typeof u === 'object') {
    for (const [k, label] of [
      ['input_tokens', 'in'],
      ['output_tokens', 'out'],
      ['cache_read_input_tokens', 'cache_read'],
      ['cache_creation_input_tokens', 'cache_write'],
    ] as const) {
      if (typeof u[k] === 'number') parts.push(`${label}=${u[k]}`)
    }
  }
  return parts.join(' ')
}

/** Body text of one entry, or '' when nothing is visible under `opts`. */
export function renderBody(entry: Entry, kind: Kind, opts: RenderOptions): string {
  const full = opts.detail === 'full'
  switch (kind) {
    case 'user':
    case 'meta':
    case 'compact_summary': {
      const out: string[] = [promptText(entry)]
      const bs = blocks(entry)
      if (bs?.some((b) => b.type === 'image')) out.push('[image]')
      return out.filter(Boolean).join('\n')
    }
    case 'assistant': {
      const out: string[] = []
      for (const b of blocks(entry) ?? []) {
        if (b.type === 'text') out.push(str(b.text))
        else if (b.type === 'thinking' && (opts.includeThinking || full)) out.push(`[thinking]\n${str(b.thinking)}`)
        else if (b.type === 'redacted_thinking' && full) out.push('[redacted thinking]')
        else if (b.type === 'tool_use') {
          const input = JSON.stringify(b.input ?? {})
          out.push(
            opts.includeTools || full
              ? `→ ${str(b.name)} [${str(b.id)}]\n${JSON.stringify(b.input ?? {}, null, 2)}`
              : `→ ${str(b.name)} ${oneLine(input, 200)}`,
          )
        }
      }
      return out.filter(Boolean).join('\n')
    }
    case 'tool_result': {
      const out: string[] = []
      for (const b of blocks(entry) ?? []) {
        if (b.type === 'tool_result') {
          out.push(
            `← result [${str(b.tool_use_id)}]${b.is_error === true ? ' (error)' : ''}\n${toolResultText(b.content)}`,
          )
        } else if (b.type === 'text') out.push(str(b.text))
      }
      return out.join('\n')
    }
    case 'compact_boundary': {
      const meta = entry.compactMetadata as { trigger?: unknown; preTokens?: unknown } | undefined
      const bits = [
        meta?.trigger ? `trigger=${String(meta.trigger)}` : '',
        meta?.preTokens ? `preTokens=${String(meta.preTokens)}` : '',
      ]
      return `--- compact boundary ${bits.filter(Boolean).join(' ')}`.trim()
    }
    case 'system':
      return [str(entry.subtype) ? `(${str(entry.subtype)})` : '', str(entry.content)].filter(Boolean).join(' ')
    case 'attachment': {
      const a = entry.attachment as { type?: unknown } | undefined
      return full ? JSON.stringify(entry.attachment ?? {}, null, 2) : `[attachment ${str(a?.type) || 'unknown'}]`
    }
    default:
      return full ? JSON.stringify(entry) : ''
  }
}

export function renderHeader(index: number, entry: Entry, kind: Kind, opts: RenderOptions): string {
  const parts = [`#${index}`, timestampOf(entry) || '-', kind]
  if (kind === 'other' && typeof entry.type === 'string') parts.push(`type=${entry.type}`)
  if (kind === 'assistant') {
    const usage = usageLine(entry)
    if (usage && opts.detail === 'full') parts.push(usage)
    else {
      const model = (entry.message as { model?: unknown } | undefined)?.model
      if (typeof model === 'string') parts.push(`model=${model}`)
    }
  }
  return parts.join(' ')
}

/** Full searchable text of an entry, independent of read options. */
export function searchableText(entry: Entry, kind: Kind): string {
  return renderBody(entry, kind, { detail: 'full', includeTools: true, includeThinking: true })
}

export function parseEntry(text: string | null): Entry | null {
  if (text === null || text.trim() === '') return null
  try {
    const v = JSON.parse(text) as unknown
    return v && typeof v === 'object' && !Array.isArray(v) ? (v as Entry) : null
  } catch {
    return null
  }
}

export function inTimeRange(entry: Entry, since: number | null, until: number | null): boolean {
  if (since === null && until === null) return true
  const t = Date.parse(timestampOf(entry))
  if (Number.isNaN(t)) return false
  return (since === null || t >= since) && (until === null || t <= until)
}
