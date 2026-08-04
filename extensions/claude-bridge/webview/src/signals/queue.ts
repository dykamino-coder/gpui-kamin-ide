import { signal, computed } from '@preact/signals'
import { activeTabId } from './tabs'
import { jsonlEntriesByTab } from './jsonl'

// Claude CLI writes queue-operation entries in JSONL but *without* `content`
// in recent versions (2.1.116+). We can't reconstruct the queue text from
// that stream, so we track it locally: push on send while CLI is busy,
// pop when a matching `user` entry appears in JSONL (CLI accepted the
// message). The queue-operation stream is only a fallback for counts.

export interface LocalQueueItem {
  id: string
  content: string
  enqueuedAt: number
  /** Highest `_ord` this tab held when the item was queued. Only rows ABOVE it
   *  can be the CLI accepting it.
   *
   *  Neither timestamps nor array positions work here. The CLI stamps entries
   *  with its own clock while `enqueuedAt` comes from ours, so a timestamp
   *  comparison needs slack — and slack let the PREVIOUS message's own user row
   *  consume an item queued a second later. An array index is exact but not
   *  STABLE: `enforceTabCap` prunes the window on every append, so recorded
   *  positions slide out from under the item and its match is never found — it
   *  then sat in the widget after the CLI had plainly answered it. `_ord` is
   *  assigned by the server in file order and survives pruning. */
  afterOrd: number
}

export const localQueue = signal<Map<string, LocalQueueItem[]>>(new Map())

/** Highest `_ord` in a tab's store, or -1. Rows without one (live stubs) don't
 *  move the mark — they are newer by construction and stay eligible. */
function maxOrd(entries: ReadonlyArray<unknown>): number {
  let max = -1
  for (const e of entries) {
    const ord = (e as { _ord?: number })._ord
    if (typeof ord === 'number' && ord > max) max = ord
  }
  return max
}

/** Does the message carry prose of its own, or is it just attachment paths?
 *  Only the latter may be cleared without a text match — the CLI replaces such
 *  a prompt with its own wording ("[Image #1]"), leaving nothing to compare. */
function hasOwnText(content: string): boolean {
  const prose = content
    .split('\n')
    .filter((l) => !/^(?:\/|[A-Za-z]:\\).*\.(?:png|jpg|jpeg|gif|webp|svg|pdf)$/i.test(l.trim()))
    .join(' ')
    .trim()
  return prose.length >= 8
}

function nextId(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

export function enqueueLocal(tabId: string, content: string): void {
  const next = new Map(localQueue.value)
  const list = [...(next.get(tabId) ?? [])]
  const afterOrd = maxOrd(jsonlEntriesByTab.value.get(tabId) ?? [])
  list.push({ id: nextId(), content, enqueuedAt: Date.now(), afterOrd })
  next.set(tabId, list)
  localQueue.value = next
}

export function clearLocalQueue(tabId: string): void {
  const next = new Map(localQueue.value)
  next.delete(tabId)
  localQueue.value = next
}

// Drop a queued item when its text appears as a user message in JSONL.
// Matches by exact content (trim-compared) — if CLI rewrote the text it
// stays in the queue until the 10-minute TTL sweep below.
export function reconcileQueueWithEntries(tabId: string): void {
  const list = localQueue.value.get(tabId)
  if (!list || list.length === 0) return
  const entries = jsonlEntriesByTab.value.get(tabId) ?? []

  // Strategy: for each queued item, look for a submission row that arrived
  // AFTER it was queued (`afterOrd` — not a timestamp or a position; see the
  // field's note). The CLI records text verbatim but whitespace and line-endings
  // can differ, so the match is loose (includes/startsWith after collapsing runs
  // of whitespace). Fallback: no text match but an unconsumed later submission
  // exists → pop FIFO (one submission consumes the oldest queued item).
  function norm(s: string): string {
    return s.replace(/\s+/g, ' ').trim().slice(0, 200)
  }

  // `submitted` = this row is the CLI having ACCEPTED something the user sent.
  // `kind` matters for the FIFO fallback below: only a row that is certainly a
  // user submission may consume a queued item without a text match.
  const userTexts: Array<{ text: string; ord: number; kind: 'user' | 'command' | 'queued' }> = []
  for (const e of entries) {
    if (!e) continue
    // No `_ord` (a live stream stub) = newer than anything already stored.
    const rawOrd = (e as { _ord?: number })._ord
    const ord = typeof rawOrd === 'number' ? rawOrd : Number.MAX_SAFE_INTEGER
    // Slash commands land in JSONL as a `system` row carrying `<command-name>`.
    // Matching ANY system row with string content (what this used to do) meant
    // routine notices — "Reloaded skills: 82 skills available" and friends —
    // counted as the CLI accepting the message, so a queued item vanished from
    // the widget while the CLI was still busy. That was the flaky queue display.
    // CLI 2.1.116+ writes queued-from-UI prompts as type:"attachment" with
    // attachment.type:"queued_command" + prompt.
    const isUser = e.type === 'user' && !(e as any).isMeta
    const content = (e as any).content
    const isLocalCmd = e.type === 'system' && typeof content === 'string' && /<command-name>/i.test(content)
    const att = (e as any).attachment
    const isQueuedCmd = e.type === 'attachment' && att?.type === 'queued_command' && typeof att.prompt === 'string'
    if (!isUser && !isLocalCmd && !isQueuedCmd) continue

    if (isQueuedCmd) {
      userTexts.push({ text: norm(att.prompt), ord, kind: 'queued' })
      continue
    }

    if (isLocalCmd) {
      userTexts.push({ text: norm(content), ord, kind: 'command' })
      continue
    }

    const msg = e.message?.content
    if (typeof msg === 'string') userTexts.push({ text: norm(msg), ord, kind: 'user' })
    else if (Array.isArray(msg)) {
      for (const b of msg) {
        if (b && typeof b === 'object' && (b as any).type === 'text' && typeof (b as any).text === 'string') {
          userTexts.push({ text: norm((b as any).text), ord, kind: 'user' })
        }
      }
    }
  }
  if (userTexts.length === 0) return

  const kept: LocalQueueItem[] = []
  let cursor = 0
  for (const item of list) {
    const itemNorm = norm(item.content)
    let consumed = false
    // Look for any user entry that appeared after this item was enqueued and
    // whose text loosely contains / is contained by ours.
    for (let i = cursor; i < userTexts.length; i++) {
      const u = userTexts[i]!
      if (u.ord <= item.afterOrd) continue
      if (
        u.text === itemNorm ||
        (itemNorm.length >= 20 && u.text.includes(itemNorm.slice(0, 40))) ||
        (u.text.length >= 20 && itemNorm.includes(u.text.slice(0, 40)))
      ) {
        cursor = i + 1
        consumed = true
        break
      }
    }
    // FIFO fallback, for the ONE case text matching cannot serve: a message the
    // CLI rewrites wholesale, i.e. an image-only prompt. Anything with real text
    // of its own must match on that text.
    //
    // Applying it to every item made it eat them: the user row for the message
    // this item was queued BEHIND is written when that turn starts — which is
    // after the item was queued — so by any "newer than" rule it looks like the
    // acceptance. That emptied the widget seconds after a message was queued.
    if (!consumed && !hasOwnText(item.content)) {
      for (let i = cursor; i < userTexts.length; i++) {
        const u = userTexts[i]!
        if (u.kind === 'command') continue
        if (u.ord > item.afterOrd) {
          cursor = i + 1
          consumed = true
          break
        }
      }
    }
    if (!consumed) kept.push(item)
  }

  if (kept.length === list.length) return
  const next = new Map(localQueue.value)
  if (kept.length === 0) next.delete(tabId)
  else next.set(tabId, kept)
  localQueue.value = next
}

// TTL sweep: anything older than 10 minutes without a matching JSONL entry
// is likely stale (user aborted, CLI crashed, etc.) — drop it.
const QUEUE_TTL_MS = 10 * 60 * 1000
export function sweepStaleQueue(): void {
  const now = Date.now()
  let changed = false
  const next = new Map(localQueue.value)
  for (const [tabId, list] of next) {
    const fresh = list.filter(i => now - i.enqueuedAt < QUEUE_TTL_MS)
    if (fresh.length === list.length) continue
    changed = true
    if (fresh.length === 0) next.delete(tabId)
    else next.set(tabId, fresh)
  }
  if (changed) localQueue.value = next
}

export const activeLocalQueue = computed<LocalQueueItem[]>(() => {
  const id = activeTabId.value
  if (!id) return []
  return localQueue.value.get(id) ?? []
})
