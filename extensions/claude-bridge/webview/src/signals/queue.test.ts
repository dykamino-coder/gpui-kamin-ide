import { describe, it, expect, beforeEach } from 'vitest'
import { localQueue, enqueueLocal, clearLocalQueue, reconcileQueueWithEntries } from './queue'
import { jsonlEntriesByTab } from './jsonl'
import type { JsonlEntryData } from '../types/jsonl'

const TAB = 't1'
const AFTER = new Date(Date.now() + 1000).toISOString()
let ord = 100
/** Entries carry `_ord` like the server's do; the queue anchors on it. */
function row(o: Record<string, unknown>): Record<string, unknown> {
  return { _ord: ++ord, timestamp: AFTER, ...o }
}

function setEntries(entries: unknown[]): void {
  jsonlEntriesByTab.value = new Map([[TAB, entries as JsonlEntryData[]]])
}
const queued = (): number => localQueue.value.get(TAB)?.length ?? 0

describe('reconcileQueueWithEntries', () => {
  beforeEach(() => {
    clearLocalQueue(TAB)
    jsonlEntriesByTab.value = new Map()
  })

  it('clears an item once the CLI records it as a user message', () => {
    enqueueLocal(TAB, 'уточни где виджет по траншам')
    setEntries([row({ type: 'user', message: { content: 'уточни где виджет по траншам' } })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(0)
  })

  it('is not consumed by the PREVIOUS message it was queued behind', () => {
    // Queue a message one second after sending another: the first message's own
    // user row is already in the stream. Matching on timestamps needed slack for
    // clock skew, and that slack let this row eat the new item — the queue
    // widget flashed and emptied straight away.
    setEntries([row({ type: 'user', message: { content: 'первое сообщение, уже принято' } })])
    enqueueLocal(TAB, 'второе сообщение, должно ждать')
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(1)
  })

  it('keeps the item when a routine system notice arrives', () => {
    // "Reloaded skills: 82 skills available" and friends are NOT the CLI
    // accepting the message — treating them as such made queued items vanish
    // from the widget while the CLI was still busy.
    enqueueLocal(TAB, 'уточни где виджет по траншам')
    setEntries([row({ type: 'system', content: 'Reloaded skills: 82 skills available (1 added)' })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(1)
  })

  it('does not let a slash-command echo consume an unrelated queued item', () => {
    enqueueLocal(TAB, 'совсем другой текст сообщения в очереди')
    setEntries([row({ type: 'system', content: '<command-name>/context</command-name>' })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(1)
  })

  it('clears a queued slash command when its own echo lands', () => {
    enqueueLocal(TAB, '<command-name>/compact</command-name>')
    setEntries([row({ type: 'system', content: '<command-name>/compact</command-name>' })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(0)
  })

  it('clears an image-only message via the FIFO fallback', () => {
    // The CLI rewrites such prompts, so no text overlap exists — but a real
    // user submission newer than the item still means it was taken.
    enqueueLocal(TAB, 'C:\\Temp\\claude-paste-1784810298099.png')
    setEntries([row({ type: 'user', message: { content: '[Image #1]' } })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(0)
  })

  it('ignores meta user rows', () => {
    enqueueLocal(TAB, 'сообщение в очереди')
    setEntries([row({ type: 'user', isMeta: true, message: { content: 'caveat' } })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(1)
  })

  it('ignores tool_result user rows (no text blocks)', () => {
    enqueueLocal(TAB, 'сообщение в очереди')
    setEntries([row({ type: 'user', message: { content: [{ type: 'tool_result', content: 'ok' }] } })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(1)
  })

  it('is not consumed by an unrelated later user row', () => {
    // The row for the message this item was queued BEHIND is written when that
    // turn starts — after the enqueue — so any "newer than" rule sees it as the
    // acceptance. A message with text of its own must match on that text.
    enqueueLocal(TAB, 'это сообщение должно дождаться своей очереди')
    setEntries([row({ type: 'user', message: { content: 'совершенно другой предыдущий вопрос' } })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(1)
  })

  it('survives the window being pruned under it', () => {
    // `enforceTabCap` trims the store on every append, so an anchor recorded as
    // an ARRAY POSITION slides out from under the item and its match is never
    // found — it then sat in the widget after the CLI had plainly answered it.
    setEntries([row({ type: 'user', message: { content: 'старая запись' } })])
    enqueueLocal(TAB, 'сообщение в очереди, ждёт ответа')
    // The store is pruned: the old row is gone, a NEWER answer takes its place.
    setEntries([row({ type: 'user', message: { content: 'сообщение в очереди, ждёт ответа' } })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(0)
  })

  it('clears items accepted as queued_command attachments', () => {
    enqueueLocal(TAB, 'уточни где виджет по траншам')
    setEntries([row({ type: 'attachment', attachment: { type: 'queued_command', prompt: 'уточни где виджет по траншам' } })])
    reconcileQueueWithEntries(TAB)
    expect(queued()).toBe(0)
  })
})
