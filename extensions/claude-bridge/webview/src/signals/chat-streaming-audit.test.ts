// Live-turn streaming in the chat-render audit (extensions/claude-bridge/docs/chat-pipeline.md §4, §7).
//
// The MITM proxy streams an assistant turn as a stub (`stream-<msgId>`, deltas
// appended in place) BEFORE the CLI writes it to the transcript; the canonical
// rows then replace it by message.id. Invariants checked here:
//  - while streaming, the live text is on screen exactly once, after the
//    last canonical message;
//  - the hand-off replaces the stub exactly once: no duplicate bubble, no lost
//    prefix, and the final text is byte-equal to the transcript's;
//  - interrupt, reconnect, reload, parallel subagents and compaction mid-stream
//    keep both properties.
import { describe, it, expect, beforeEach, vi } from 'vitest'
import {
  appendJsonlEntries, applyStreamingEntry, applyStreamingDelta, abortStreamingForTab, clearJsonlEntries, getJsonlEntries,
} from './jsonl'
import { activeTabId } from './tabs'
import { compactingTabs, noteCompactionRows } from './compaction'
import { TranscriptBuilder, conversation, asPaged, asLive, type Rec } from './chat-audit/transcript'
import { cliView, chatView } from './chat-audit/views'
import type { JsonlEntryData } from '../types/jsonl'

let n = 0
let TAB = ''
// One fake clock for the whole file: the stub sweeper is a module-level interval,
// so it must be created under the same clock every test advances.
vi.useFakeTimers()
beforeEach(() => {
  TAB = `stream-${String(++n)}`
  activeTabId.value = TAB
  clearJsonlEntries(TAB)
})

/** The proxy's opening snapshot: thinking + text blocks, both empty (content_block_start). */
const stub = (msgId: string, text = '', extra: Partial<JsonlEntryData> = {}): JsonlEntryData =>
  ({ type: 'assistant', uuid: `stream-${msgId}`, __streaming: true,
    message: { id: msgId, role: 'assistant', content: [{ type: 'thinking', thinking: '' }, { type: 'text', text }] }, ...extra }) as unknown as JsonlEntryData
const delta = (msgId: string, appendText: string) => { applyStreamingDelta(TAB, { msgId, blockIdx: 1, appendText, kind: 'text' }) }
const streamText = (msgId: string, text: string, pieces = 7): void => {
  const step = Math.max(1, Math.ceil(text.length / pieces))
  for (let i = 0; i < text.length; i += step) delta(msgId, text.slice(i, i + step))
}
/** Rows the builder appended since `from`. */
const since = (b: TranscriptBuilder, from: number): Rec[] => b.recs.slice(from)
const lastA = (): string | undefined => chatView(TAB).all.filter((l) => l.startsWith('A:')).at(-1)
const countOf = (text: string): number => chatView(TAB).all.filter((l) => l === `A:${text}`).length

/** History, then a user prompt whose answer is about to stream. */
function setup(): { b: TranscriptBuilder; mark: number } {
  // A fresh id prefix per test: message ids must be unique like real API ids
  // (the interrupt freeze-set is keyed by message.id across tabs).
  const b = conversation(3, TAB)
  b.user('stream me an answer')
  deliver(asPaged(b.recs))
  return { b, mark: b.recs.length }
}
const deliver = (rows: JsonlEntryData[]): void => { appendJsonlEntries(TAB, rows) }
const msgIdOf = (b: TranscriptBuilder, from: number): string => since(b, from).find((r) => r.type === 'assistant')!.message!.id!

describe('stream, then the canonical hand-off', () => {
  it('live text shows once at the tail and the final text is byte-equal to the transcript', () => {
    const { b, mark } = setup()
    const text = 'Hello — streamed\n\n```ts\nconst x = 1\n```\nend.'
    b.say(text)
    const id = msgIdOf(b, mark)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, text)
    expect(lastA()).toBe(`A:${text}`)
    expect(chatView(TAB).all.at(-1)).toBe(`A:${text}`)
    deliver(asLive(since(b, mark)))
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
    expect(countOf(text)).toBe(1)
    expect(getJsonlEntries(TAB).some((e) => e.__streaming !== undefined)).toBe(false)
  })

  // D5 / INC-2026-0088 (#673/#675): the CLI writes the thinking row first. Merging
  // it into the stub used to drop the streamed prose until the text row landed.
  it('the thinking-only canonical row does not blank the streamed prose (D5, INC-2026-0088)', () => {
    const { b, mark } = setup()
    b.say('visible prose')
    const id = msgIdOf(b, mark)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, 'visible prose')
    const [thinkingRow] = asLive(since(b, mark))
    deliver([thinkingRow!])
    expect(lastA()).toBe('A:visible prose')
    expect(countOf('visible prose')).toBe(1)
    deliver(asLive(since(b, mark)).slice(1))
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
    expect(getJsonlEntries(TAB).some((e) => e.__streaming !== undefined)).toBe(false)
  })

  it('rows held under the stub survive a later stub snapshot and the TTL sweep', () => {
    const { b, mark } = setup()
    b.say('held prose')
    const id = msgIdOf(b, mark)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, 'held prose')
    const [thinkingRow, textRow] = asLive(since(b, mark))
    deliver([thinkingRow!])
    applyStreamingEntry(TAB, stub(id, 'held prose')) // snapshot after the hold
    b.user('next prompt') // a later turn supersedes the stub
    deliver(asLive(b.recs.slice(-1)))
    vi.advanceTimersByTime(30_000) // swept: the held thinking row must stay in the store
    expect(getJsonlEntries(TAB).some((e) => e.uuid === thinkingRow!.uuid || e.__uuids?.includes(thinkingRow!.uuid!))).toBe(true)
    deliver([textRow!])
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
  })

  it('a late stub snapshot after the canonical rows changes nothing', () => {
    const { b, mark } = setup()
    b.say('final words')
    const id = msgIdOf(b, mark)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, 'final wo')
    deliver(asLive(since(b, mark)))
    applyStreamingEntry(TAB, stub(id, 'final wo')) // throttled flush that lost the race
    delta(id, 'rds')
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
  })

  it('a 100 KB answer streamed in 2 000 deltas ends byte-equal', () => {
    const { b, mark } = setup()
    const text = Array.from({ length: 2_000 }, (_, i) => `chunk ${String(i)} ${'y'.repeat(40)}`).join('\n')
    b.say(text)
    const id = msgIdOf(b, mark)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, text, 2_000)
    expect(lastA()).toBe(`A:${text}`)
    deliver(asLive(since(b, mark)))
    expect(lastA()).toBe(`A:${text}`)
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
  })
})

describe('mid-stream disruptions', () => {
  it('interrupt: later frames are dropped, the committed partial wins', () => {
    const { b, mark } = setup()
    b.interrupted('I was in the mid')
    const id = msgIdOf(b, mark)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, 'I was in the mid')
    abortStreamingForTab(TAB)
    delta(id, 'dle of something') // proxy keeps flushing buffered frames after SIGINT
    expect(lastA()).toBe('A:I was in the mid')
    deliver(asLive(since(b, mark)))
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
  })

  it('connection drop and reconnect: the re-emitted snapshot replaces the stub, no duplicate', () => {
    const { b, mark } = setup()
    b.say('one two three four')
    const id = msgIdOf(b, mark)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, 'one two')
    applyStreamingEntry(TAB, stub(id, 'one two three')) // host cachedStreamingEntry after reconnect
    delta(id, ' four')
    expect(chatView(TAB).all.filter((l) => l.startsWith('A:one'))).toEqual(['A:one two three four'])
    deliver(asLive(since(b, mark)))
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
  })

  it('webview reload mid-stream: replay, then the cached stub, then the canonical rows', () => {
    const { b, mark } = setup()
    b.say('answer after reload')
    const id = msgIdOf(b, mark)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, 'answer after')
    clearJsonlEntries(TAB)
    deliver(asPaged(b.recs.slice(0, mark))) // replay of what the file already holds
    applyStreamingEntry(TAB, stub(id, 'answer after')) // re-emitted LAST by replayJsonlToRenderer
    delta(id, ' reload')
    expect(chatView(TAB).all.at(-1)).toBe('A:answer after reload')
    deliver(asLive(since(b, mark)))
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
  })

  it('parallel subagent streams stay out of the main chat', () => {
    const { b, mark } = setup()
    b.sidechain('subagent result')
    b.say('main answer')
    const main = since(b, mark).find((r) => r.type === 'assistant' && !r.isSidechain)!.message!.id!
    applyStreamingEntry(TAB, stub('side-a', '', { isSidechain: true }))
    applyStreamingEntry(TAB, stub(main))
    applyStreamingEntry(TAB, stub('side-b', '', { isSidechain: true }))
    applyStreamingDelta(TAB, { msgId: 'side-a', blockIdx: 1, appendText: 'subagent result', kind: 'text' })
    streamText(main, 'main answer')
    expect(chatView(TAB).all.at(-1)).toBe('A:main answer')
    expect(chatView(TAB).all.join()).not.toContain('subagent')
    deliver(asLive(since(b, mark)))
    expect(chatView(TAB).all.join()).not.toContain('subagent')
    expect(chatView(TAB).all.at(-1)).toBe('A:main answer')
  })

  it('compaction while streaming: clear, replay the new segment, the turn still lands once', () => {
    const { b, mark } = setup()
    b.compact('summary written mid-turn')
    const afterCompact = b.recs.length
    b.say('answer that straddles compaction')
    const id = msgIdOf(b, afterCompact)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, 'answer that')
    clearJsonlEntries(TAB) // `compacted` status
    deliver(asPaged(b.recs.slice(0, afterCompact)))
    applyStreamingEntry(TAB, stub(id, 'answer that'))
    delta(id, ' straddles compaction')
    expect(chatView(TAB).all.at(-1)).toBe('A:answer that straddles compaction')
    deliver(asLive(since(b, afterCompact)))
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
    void mark
  })

  // D6: orderEntries sorts every stub after all canonical rows, so a successor
  // is recognised by its later CLI timestamp, not by its position.
  it('API retry with a new message id: the superseded stub is swept, only the retry stays (D6)', () => {
    const { b, mark } = setup()
    applyStreamingEntry(TAB, stub('msg-aborted'))
    applyStreamingDelta(TAB, { msgId: 'msg-aborted', blockIdx: 1, appendText: 'first attempt', kind: 'text' })
    b.say('second attempt')
    deliver(asLive(since(b, mark)))
    vi.advanceTimersByTime(30_000)
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
  })
})

// The CLI's own summary request (manual /compact or auto-compact). The server
// sends only a contentless `__compaction` marker for it (compaction-request.ts);
// the CLI TUI shows "Compacting conversation…" and never the summary text.
describe('compaction request stream', () => {
  const marker = (msgId: string, streaming: boolean): JsonlEntryData =>
    ({ type: 'assistant', uuid: `stream-${msgId}`, __streaming: streaming, __compaction: true,
      message: { id: msgId, role: 'assistant', content: [] } }) as unknown as JsonlEntryData
  const compacting = () => compactingTabs.value.has(TAB)
  const deliverRows = (rows: JsonlEntryData[]) => { deliver(rows); noteCompactionRows(TAB, rows) }

  it('shows the indicator, never an answer, and leaves no stub after the compaction', () => {
    const { b } = setup()
    const before = chatView(TAB).all
    applyStreamingEntry(TAB, marker(`${TAB}-compact`, true))
    applyStreamingDelta(TAB, { msgId: `${TAB}-compact`, blockIdx: 0, appendText: '<analysis> Let me analyze', kind: 'text' })
    expect(compacting()).toBe(true)
    expect(chatView(TAB).all).toEqual(before)
    applyStreamingEntry(TAB, marker(`${TAB}-compact`, false))
    expect(compacting()).toBe(false)
    const mark = b.recs.length
    b.compact('the summary')
    clearJsonlEntries(TAB) // `compacted` status
    deliverRows(asPaged(b.recs.slice(mark)))
    expect(getJsonlEntries(TAB).some((row) => row.__streaming !== undefined)).toBe(false)
    expect(chatView(TAB).all).toEqual(cliView(b.recs))
  })

  it('a lost final snapshot ends with the compaction result', () => {
    const { b } = setup()
    applyStreamingEntry(TAB, marker(`${TAB}-lost`, true))
    deliverRows(asLive([b.user('unrelated live row')]))
    expect(compacting()).toBe(true)
    const mark = b.recs.length
    b.compact('summary after a reconnect')
    deliverRows(asLive(b.recs.slice(mark)))
    expect(compacting()).toBe(false)
  })

  it('a normal turn quoting the summary text still streams as an answer', () => {
    const { b, mark } = setup()
    const text = '<analysis> is how a compaction summary starts'
    b.say(text)
    const id = msgIdOf(b, mark)
    applyStreamingEntry(TAB, stub(id))
    streamText(id, text)
    expect(compacting()).toBe(false)
    expect(lastA()).toBe(`A:${text}`)
  })
})
