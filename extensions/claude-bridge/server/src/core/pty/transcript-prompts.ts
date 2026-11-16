// Which transcript records are stored prompts, and whether one stores a given text.
// The same rows and the same comparison as the clients' queue matcher
// (webview/shared/submission-queue.ts), so a receipt and the transcript agree.
import type { JsonlEntry } from '../../shared/jsonl-types'

type Row = {
  type?: string
  uuid?: string
  timestamp?: string
  isMeta?: boolean
  isSidechain?: boolean
  agentId?: string
  message?: { content?: unknown }
  attachment?: { type?: string; prompt?: unknown }
}

export interface PromptRow {
  uuid: string
  text: string
  /** CLI time of the record in ms, or NaN without one. */
  at: number
}

export function normalizePrompt(text: string): string {
  return text.replace(/\s+/g, ' ').trim().slice(0, 200)
}

/** The main-thread prompt a record stores: a prompt line, or a prompt the
 * CLI delivered mid-turn from its queue. Tool results, meta rows and
 * sub-agent rows are not prompts. */
export function promptRow(entry: JsonlEntry): PromptRow | undefined {
  const row = entry as unknown as Row
  if (typeof row.uuid !== 'string' || row.isSidechain || row.agentId) return undefined
  let text: string | undefined
  if (row.type === 'attachment') {
    if (row.attachment?.type === 'queued_command' && typeof row.attachment.prompt === 'string')
      text = row.attachment.prompt
  } else if (row.type === 'user' && !row.isMeta) {
    const content = row.message?.content
    if (typeof content === 'string') text = content
    else if (Array.isArray(content) && !content.some((block) => block?.type === 'tool_result')) {
      const blocks = content.filter((block) => block?.type === 'text' && typeof block.text === 'string')
      if (blocks.length) text = blocks.map((block) => block.text as string).join('\n')
    }
  }
  if (text === undefined) return undefined
  return { uuid: row.uuid, text: normalizePrompt(text), at: row.timestamp ? Date.parse(row.timestamp) : NaN }
}

/** Equal after whitespace folding; a long text may be stored with its start. */
export function storesPrompt(row: PromptRow, normalized: string): boolean {
  return (
    row.text === normalized ||
    (normalized.length >= 20 && row.text.includes(normalized.slice(0, 40))) ||
    (row.text.length >= 20 && normalized.includes(row.text.slice(0, 40)))
  )
}
