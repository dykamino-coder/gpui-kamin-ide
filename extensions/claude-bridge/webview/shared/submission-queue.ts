export interface SubmissionUpdate {
  /** Browser host сохраняет курсор legacy queue независимо от страниц. */
  fallbackQueue?: { afterOrd: number; enqueuedAt: number; consumed?: boolean }
  id: string
  text?: string
  state?: 'queued' | 'sending' | 'native' | 'read' | 'cancelled' | 'uncertain' | 'fallback'
  uuid?: string
  cancelable?: boolean
  warning?: string
  capable?: boolean
}

export interface LocalQueueItem {
  id: string
  content: string
  enqueuedAt: number
  /** Только строки после этой позиции могут подтвердить отправку. */
  afterOrd: number
  /** Browser queue отображает решение host, desktop использует локальный matcher. */
  hostOwned?: boolean
}

export interface QueueEntry {
  type: string
  uuid?: string
  _ord?: number
  isMeta?: boolean
  content?: unknown
  message?: { content: unknown }
  attachment?: { type?: string; prompt?: string }
}

export function queueWatermark(entries: readonly QueueEntry[]): number {
  return entries.reduce((max, row) => Math.max(max, row._ord ?? -1), -1)
}

export function queueRowIdentity(row: QueueEntry): string | undefined {
  if (row.uuid) return `uuid:${row.uuid}`
  if (typeof row._ord === 'number') return `ord:${row._ord}`
}

function norm(text: string): string {
  return text.replace(/\s+/g, ' ').trim().slice(0, 200)
}

function hasOwnText(content: string): boolean {
  return content.split('\n')
    .filter(line => !/^(?:\/|[A-Za-z]:\\).*\.(?:png|jpg|jpeg|gif|webp|svg|pdf)$/i.test(line.trim()))
    .join(' ').trim().length >= 8
}

/** Один matcher для desktop queue и headless host; системные notices и
 * tool_result не подтверждают отправку. Одна строка принимает только один item. */
export function matchSubmissionQueue(
  items: readonly LocalQueueItem[], entries: readonly QueueEntry[], consumed: ReadonlySet<string> = new Set(),
): Array<{ item: LocalQueueItem; entry: QueueEntry }> {
  const rows: Array<{ entry: QueueEntry; text: string; ord: number; kind: 'user' | 'command' | 'queued' }> = []
  const seen = new Set<string>()
  for (const entry of entries) {
    const identity = queueRowIdentity(entry)
    if (identity && (consumed.has(identity) || seen.has(identity))) continue
    if (identity) seen.add(identity)
    const ord = entry._ord ?? Number.MAX_SAFE_INTEGER
    if (entry.type === 'attachment' && entry.attachment?.type === 'queued_command' && typeof entry.attachment.prompt === 'string') {
      rows.push({ entry, text: norm(entry.attachment.prompt), ord, kind: 'queued' })
    } else if (entry.type === 'system' && typeof entry.content === 'string' && /<command-name>/i.test(entry.content)) {
      rows.push({ entry, text: norm(entry.content), ord, kind: 'command' })
    } else if (entry.type === 'user' && !entry.isMeta) {
      const content = entry.message?.content
      const blocks = Array.isArray(content) ? content.filter(block => block?.type === 'text' && typeof block.text === 'string') : []
      const text = typeof content === 'string' ? content : blocks.length ? blocks.map(block => block.text).join('\n') : undefined
      if (text !== undefined) rows.push({ entry, text: norm(text), ord, kind: 'user' })
    }
  }
  const matches: Array<{ item: LocalQueueItem; entry: QueueEntry }> = []
  let cursor = 0
  for (const item of items) {
    const text = norm(item.content)
    let index = rows.findIndex((row, i) => i >= cursor && row.ord > item.afterOrd && (
      row.text === text || (text.length >= 20 && row.text.includes(text.slice(0, 40))) ||
      (row.text.length >= 20 && text.includes(row.text.slice(0, 40)))
    ))
    // CLI переписывает image-only prompt; только для него допустим FIFO.
    if (index < 0 && !hasOwnText(item.content)) index = rows.findIndex((row, i) => i >= cursor && row.kind !== 'command' && row.ord > item.afterOrd)
    if (index >= 0) {
      matches.push({ item, entry: rows[index]!.entry })
      cursor = index + 1
    }
  }
  return matches
}
