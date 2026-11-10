export interface SubmissionUpdate {
  /** Host observed the stored UUID; survives eviction from replay windows. */
  committed?: boolean
  /** Browser host сохраняет курсор legacy queue независимо от страниц. */
  fallbackQueue?: { afterOrd: number; enqueuedAt: number; consumed?: boolean; dismissed?: boolean }
  id: string
  text?: string
  state?: 'queued' | 'sending' | 'native' | 'read' | 'cancelled' | 'uncertain' | 'fallback'
  uuid?: string
  cancelable?: boolean
  warning?: string
  capable?: boolean
  /** Server time (ms) the server first accepted this id; same clock as the CLI
   *  transcript timestamps, so a transcript row can be checked against it. */
  createdAt?: number
  /** Reattach snapshot (with `id: ''`): every id the server still tracks. A
   *  pending receipt missing from it was lost by a server or CLI restart. */
  known?: string[]
}

// Поздний receipt не должен возвращать очередь после Escape или принятого JSONL.
export function mergeFallbackQueue(previous: SubmissionUpdate['fallbackQueue'], update: SubmissionUpdate['fallbackQueue']): SubmissionUpdate['fallbackQueue'] {
  if (!previous) return update
  return { ...previous, ...update, ...(previous.consumed ? { consumed: true } : {}),
    ...(previous.dismissed ? { dismissed: true } : {}) }
}

export interface LocalQueueItem {
  id: string
  content: string
  enqueuedAt: number
  /** Только строки после этой позиции могут подтвердить отправку. */
  afterOrd: number
  /** Browser queue отображает решение host, desktop использует локальный matcher. */
  hostOwned?: boolean
  /** Server time (ms): a row with an older CLI timestamp cannot confirm it. */
  notBefore?: number
}

export interface QueueEntry {
  type: string
  uuid?: string
  _ord?: number
  timestamp?: string
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
  const rows: Array<{ entry: QueueEntry; text: string; ord: number; ts: number; kind: 'user' | 'command' | 'queued' }> = []
  const seen = new Set<string>()
  for (const entry of entries) {
    const identity = queueRowIdentity(entry)
    if (identity && (consumed.has(identity) || seen.has(identity))) continue
    if (identity) seen.add(identity)
    const ord = entry._ord ?? Number.MAX_SAFE_INTEGER
    const ts = entry.timestamp ? Date.parse(entry.timestamp) : NaN
    if (entry.type === 'attachment' && entry.attachment?.type === 'queued_command' && typeof entry.attachment.prompt === 'string') {
      rows.push({ entry, text: norm(entry.attachment.prompt), ord, ts, kind: 'queued' })
    } else if (entry.type === 'system' && typeof entry.content === 'string' && /<command-name>/i.test(entry.content)) {
      rows.push({ entry, text: norm(entry.content), ord, ts, kind: 'command' })
    } else if (entry.type === 'user' && !entry.isMeta) {
      const content = entry.message?.content
      const blocks = Array.isArray(content) ? content.filter(block => block?.type === 'text' && typeof block.text === 'string') : []
      const text = typeof content === 'string' ? content : blocks.length ? blocks.map(block => block.text).join('\n') : undefined
      if (text !== undefined) rows.push({ entry, text: norm(text), ord, ts, kind: 'user' })
    }
  }
  const matches: Array<{ item: LocalQueueItem; entry: QueueEntry }> = []
  let cursor = 0
  for (const item of items) {
    const text = norm(item.content)
    // Rows written before the server accepted the item (its own clock) cannot
    // confirm it; a 2 s margin covers the CLI and server rounding the same instant.
    const after = (row: { ord: number; ts: number }): boolean => row.ord > item.afterOrd &&
      !(item.notBefore !== undefined && Number.isFinite(row.ts) && row.ts < item.notBefore - 2000)
    let index = rows.findIndex((row, i) => i >= cursor && after(row) && (
      row.text === text || (text.length >= 20 && row.text.includes(text.slice(0, 40))) ||
      (row.text.length >= 20 && text.includes(row.text.slice(0, 40)))
    ))
    // CLI переписывает image-only prompt; только для него допустим FIFO.
    if (index < 0 && !hasOwnText(item.content)) index = rows.findIndex((row, i) => i >= cursor && row.kind !== 'command' && after(row))
    if (index >= 0) {
      matches.push({ item, entry: rows[index]!.entry })
      cursor = index + 1
    }
  }
  return matches
}

// ── Receipts that lost their server-side confirmation ──────────────────────
// A receipt normally resolves by its stored UUID (`read` + uuid, or
// `committed`). Two ways it never does, and its bubble stays below the chat
// for days: the server or CLI restarted and forgot the id (no further update
// ever arrives), or delivery went through but the read receipt did not
// (`uncertain`, or a receipt lost while the socket was down).

const SERVER_OWNED = new Set<SubmissionUpdate['state']>(['queued', 'sending', 'native'])

export const LOST_RECEIPT_WARNING = 'Bridge server no longer tracks this message after a restart. Check the transcript before resending.'

/** Unconfirmed without a stored UUID: only the transcript can still settle it.
 *  Receipts the server still tracks wait for their read UUID instead. */
export function unresolvedReceipt(receipt: SubmissionUpdate): boolean {
  return !receipt.committed && !receipt.uuid && receipt.state === 'uncertain' && !!receipt.text
}

/** After a reattach snapshot: the server forgot this pending receipt. */
export function lostAfterSnapshot(receipt: SubmissionUpdate, known: ReadonlySet<string>): boolean {
  return !receipt.committed && !receipt.uuid && SERVER_OWNED.has(receipt.state) && !known.has(receipt.id)
}

/** Unresolved receipts confirmed by a prompt the CLI wrote after them. The same
 *  matcher as the queue: a row confirms at most one receipt, in order, and
 *  never one written before the receipt (position watermark, server time). */
export function matchReceiptsToTranscript(
  receipts: ReadonlyArray<SubmissionUpdate & { afterOrd: number }>,
  entries: readonly QueueEntry[],
  consumed: ReadonlySet<string>,
): Array<{ id: string; entry: QueueEntry }> {
  const items = receipts.filter(unresolvedReceipt).map(receipt => ({
    id: receipt.id, content: receipt.text!, enqueuedAt: 0, afterOrd: receipt.afterOrd,
    ...(receipt.createdAt !== undefined ? { notBefore: receipt.createdAt } : {}),
  }))
  // Untagged live rows carry no position, so without the server time only a
  // positioned row can be proven newer than the receipt.
  const timed = items.filter(item => item.notBefore !== undefined)
  const untimed = items.filter(item => item.notBefore === undefined)
  const used = new Set(consumed)
  const out: Array<{ id: string; entry: QueueEntry }> = []
  for (const [group, rows] of [[timed, entries], [untimed, entries.filter(row => typeof row._ord === 'number')]] as const) {
    for (const { item, entry } of matchSubmissionQueue(group, rows, used)) {
      const key = queueRowIdentity(entry)
      if (key) used.add(key)
      out.push({ id: item.id, entry })
    }
  }
  return out
}