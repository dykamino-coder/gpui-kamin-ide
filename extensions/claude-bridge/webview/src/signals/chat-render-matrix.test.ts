// Chat-render audit matrix (extensions/claude-bridge/docs/chat-pipeline.md §7). Each case delivers one
// synthetic transcript through the REAL ingest paths (append / prepend / trim /
// clear / segment replace / streaming) and compares what the chat renders with
// what the Claude Code CLI shows for the same JSONL (views.ts cliView).
//
// Invariant: every user/assistant message the CLI shows appears in the chat's
// rows exactly once and in CLI order; over the store window the chat holds a
// contiguous suffix that ends at the CLI's last message.
//
// `it.fails` marks a KNOWN defect, each with its reference. When the fix lands
// the case starts passing and vitest reports it — flip it to `it` then.
import { describe, it, expect, beforeEach } from 'vitest'
import {
  appendJsonlEntries, clearJsonlEntries, getJsonlEntries, prependJsonlEntries, trimTabToWindow,
  replaceWindowWithSegment, setArchivedView, tabNeedsRefill, STORE_WINDOW, STORE_SLACK,
} from './jsonl'
import { activeTabId } from './tabs'
import { TranscriptBuilder, conversation, asPaged, asLive, rng, chunk, type Rec } from './chat-audit/transcript'
import { cliView, chatView, isSuffix } from './chat-audit/views'
import type { JsonlEntryData } from '../types/jsonl'

let n = 0
let TAB = ''
beforeEach(() => {
  TAB = `audit-${String(++n)}`
  activeTabId.value = TAB
  clearJsonlEntries(TAB)
})

const deliver = (rows: JsonlEntryData[], rand?: () => number): void => {
  for (const batch of rand ? chunk(rows, rand) : [rows]) appendJsonlEntries(TAB, batch)
}
/** Legacy reverse replay: newest batch first, a per-process `_ord` counter, no `_pos`. */
const legacyReverse = (recs: readonly Rec[], base: number, batch = 200): void => {
  const rows = asLive(recs).map((r, i) => ({ ...r, _ord: base + i }))
  for (let end = rows.length; end > 0; end -= batch) appendJsonlEntries(TAB, rows.slice(Math.max(0, end - batch), end))
}
const expectCli = (recs: readonly Rec[]): void => { expect(chatView(TAB).all).toEqual(cliView(recs)) }

describe('delivery order and duplication', () => {
  const b = conversation(30)
  b.toolBurst('reading two files', 2)
  b.say('done reading')

  it('one paged replay', () => { deliver(asPaged(b.recs)); expectCli(b.recs) })
  it('live tail in random chunks (untagged rows)', () => { deliver(asLive(b.recs), rng(1)); expectCli(b.recs) })
  it('legacy reverse replay (newest batch first, replay-local _ord)', () => { legacyReverse(b.recs, 5_000, 7); expectCli(b.recs) })

  it('out-of-order paged batches', () => {
    const batches = chunk(asPaged(b.recs), rng(2), 9)
    const rand = rng(3)
    for (let i = batches.length - 1; i > 0; i--) { const j = Math.floor(rand() * (i + 1)); [batches[i], batches[j]] = [batches[j]!, batches[i]!] }
    for (const x of batches) appendJsonlEntries(TAB, x)
    expectCli(b.recs)
  })

  it('every batch delivered twice', () => {
    for (const x of chunk(asPaged(b.recs), rng(4))) { appendJsonlEntries(TAB, x); appendJsonlEntries(TAB, structuredClone(x)) }
    expectCli(b.recs)
  })

  it('reattach re-replays the same rows with fresh ords', () => {
    legacyReverse(b.recs, 1_000)
    legacyReverse(b.recs, 900_000)
    expectCli(b.recs)
  })
})

describe('reload, eviction and refill', () => {
  it('store clear, a post-clear live echo, then the owed refill', () => {
    const b = conversation(20)
    deliver(asPaged(b.recs))
    clearJsonlEntries(TAB) // memory eviction / compaction clear
    b.user('after the clear')
    deliver(asLive(b.recs.slice(-1))) // one live row lands before the refill
    expect(tabNeedsRefill(TAB)).toBe(true)
    deliver(asPaged(b.recs)) // the refill the viewer requests
    expectCli(b.recs)
  })

  it('trim, then a reconnect replay of the whole transcript', () => {
    const b = conversation(40)
    deliver(asPaged(b.recs))
    trimTabToWindow(TAB, 50)
    legacyReverse(b.recs, 77_000)
    expectCli(b.recs)
  })
})

describe('long sessions over the store cap', () => {
  const b = conversation(1_800) // 5 400 rows: over STORE_WINDOW + STORE_SLACK
  const cli = cliView(b.recs)

  it('background tab: capped to a contiguous suffix ending at the last message', () => {
    activeTabId.value = 'some-other-tab'
    deliver(asPaged(b.recs), rng(5))
    activeTabId.value = TAB
    expect(getJsonlEntries(TAB).length).toBeLessThanOrEqual(STORE_WINDOW + STORE_SLACK)
    const v = chatView(TAB)
    expect(isSuffix(v.all, cli)).toBe(true)
    expect(v.window.at(-1)).toBe(cli.at(-1))
    expect(isSuffix(v.window, cli)).toBe(true)
  })

  it('bottom trim then scroll-up pages restore the whole history in order', () => {
    deliver(asPaged(b.recs))
    trimTabToWindow(TAB, STORE_WINDOW)
    for (;;) { // the viewer's onNearTop → loadOlderPage(beforePos = store[0]._pos, 400)
      const before = getJsonlEntries(TAB)[0]?._pos ?? 0
      const page = b.recs.filter((r) => (r._pos ?? 0) < before).slice(-400)
      if (page.length === 0) break
      prependJsonlEntries(TAB, asPaged(page))
    }
    expect(chatView(TAB).all).toEqual(cli)
  })

  // Scroll-up asks for the page before `store[0]._pos` and gives up when the
  // oldest resident row has none (JsonlViewer onNearTop → reachedStart). Live
  // tail rows arrive untagged (server jsonl-watcher.ts), so after a bottom trim
  // of a session that arrived live there is no cursor and history above the
  // window cannot be paged back.
  it.fails('after a bottom trim of a live session the oldest resident row still has a page cursor (expected-fail: chat-pipeline.md D7)', () => {
    deliver(asLive(b.recs), rng(12))
    trimTabToWindow(TAB, STORE_WINDOW)
    expect(typeof getJsonlEntries(TAB)[0]?._pos).toBe('number')
  })

  // D3, fixed by #698: uuid-less bookkeeping rows were stored and counted by the
  // cap, so a flood after the last message evicted every real row.
  it('cost-state flood after the last message keeps the conversation (D3, #698)', () => {
    const f = conversation(10)
    f.bookkeeping(STORE_WINDOW + STORE_SLACK + 10)
    activeTabId.value = 'some-other-tab'
    deliver(asPaged(f.recs), rng(6))
    activeTabId.value = TAB
    expect(chatView(TAB).all).toEqual(cliView(f.recs))
  })
})

describe('compaction', () => {
  const b = conversation(5, 'c')
  b.compact('summary of the first conversation')
  conversationInto(b, 4, 'second')
  b.compact('summary of the second conversation')
  conversationInto(b, 3, 'third')

  it('current segment shows what the CLI resumes into (post-boundary)', () => {
    deliver(asPaged(b.recs), rng(7))
    expectCli(b.recs)
  })

  it('archived segment replaces the window; live rows wait and return with Current', () => {
    deliver(asPaged(b.recs))
    const first = b.recs.findIndex((r) => r.subtype === 'compact_boundary')
    setArchivedView(TAB, 'seg0')
    replaceWindowWithSegment(TAB, asPaged(b.recs.slice(0, first)))
    expect(chatView(TAB).all).toEqual(cliView(b.recs.slice(0, first)))
    b.user('live while archived')
    b.say('live answer')
    deliver(asLive(b.recs.slice(-3)))
    expect(chatView(TAB).all).toEqual(cliView(b.recs.slice(0, first))) // snapshot untouched
    setArchivedView(TAB, null)
    expectCli(b.recs)
  })
})

describe('turn shapes', () => {
  it('split content blocks arriving out of order across batches fold into one message', () => {
    const b = conversation(2)
    b.user('explain')
    b.assistant([{ type: 'thinking', thinking: '' }, { type: 'text', text: 'part one' }, { type: 'text', text: 'part two' }])
    const rows = asPaged(b.recs)
    deliver(rows.slice(0, -3))
    appendJsonlEntries(TAB, rows.slice(-1))
    appendJsonlEntries(TAB, rows.slice(-3, -1))
    expectCli(b.recs)
    expect(chatView(TAB).all.at(-1)).toBe('A:part one\npart two')
  })

  it('parallel tool calls: prose once, both calls in one burst', () => {
    const b = conversation(1)
    b.user('read both')
    b.toolBurst('reading', 2)
    b.say('both read')
    deliver(asLive(b.recs), rng(8))
    expectCli(b.recs)
    const calls = getJsonlEntries(TAB).flatMap((e) => Array.isArray(e.message?.content) ? e.message.content : [])
      .filter((c) => c.type === 'tool_use')
    expect(calls).toHaveLength(2)
  })

  it('sidechain/subagent rows stay out of the main chat', () => {
    const b = conversation(3)
    b.sidechain('subagent says hi')
    b.say('main continues')
    deliver(asPaged(b.recs))
    expectCli(b.recs)
    expect(chatView(TAB).all.join()).not.toContain('subagent says hi')
  })

  it('interrupted turn: partial answer shown, the marker is not a bubble', () => {
    const b = conversation(2)
    b.user('long task')
    b.interrupted('I started to')
    b.user('never mind')
    b.say('ok')
    deliver(asLive(b.recs), rng(9))
    expectCli(b.recs)
  })

  it('queued message: queue-operation rows plus the real user row render once', () => {
    const b = conversation(2)
    b.queueOperation('enqueue', 'queued follow-up')
    b.say('finishing the previous turn')
    b.queueOperation('dequeue', 'queued follow-up')
    b.user('queued follow-up')
    b.say('answering the follow-up')
    deliver(asLive(b.recs), rng(10))
    expectCli(b.recs)
  })

  it('isMeta rows and bookkeeping between turns render nothing', () => {
    const b = conversation(2)
    b.meta('<system-reminder>internal</system-reminder>')
    b.bookkeeping(30, 'api-request')
    b.say('still visible')
    deliver(asLive(b.recs), rng(11))
    expectCli(b.recs)
  })

  it('a 200 KB assistant message keeps every byte and its prefix', () => {
    const b = conversation(1)
    b.user('write a lot')
    const text = Array.from({ length: 4_000 }, (_, i) => `line ${String(i)} ${'x'.repeat(40)}`).join('\n')
    b.say(text)
    deliver(asPaged(b.recs))
    expect(chatView(TAB).all.at(-1)).toBe(`A:${text}`)
    expectCli(b.recs)
  })

  // D4: the CLI resumes ONE branch (leaf → root); a rewound-away branch must not
  // stay on screen as its own leaf (compute-chain-uuids.ts).
  it('rewind: the abandoned branch is hidden like in the CLI (D4)', () => {
    const b = conversation(2)
    const fork = b.parent!
    b.user('first try')
    b.say('first try answer')
    b.rewindTo(fork)
    b.user('second try')
    b.say('second try answer')
    deliver(asPaged(b.recs))
    expectCli(b.recs)
  })
})

describe('rewinds (D4)', () => {
  it('rewind to the start: only the new root conversation shows', () => {
    const b = new TranscriptBuilder('rw-root')
    b.user('abandoned opening')
    b.say('abandoned reply')
    b.rewindTo(undefined as unknown as string)
    b.user('fresh opening')
    b.say('fresh reply')
    deliver(asLive(b.recs), rng(13))
    expectCli(b.recs)
  })

  it('two rewinds in a row, then several turns, delivered as paged chunks', () => {
    const b = conversation(3, 'rw-two')
    const fork = b.parent!
    b.user('try one'); b.say('one')
    b.rewindTo(fork)
    b.user('try two'); b.toolBurst('checking', 2); b.say('two')
    b.rewindTo(fork)
    b.user('try three'); b.say('three')
    for (let t = 0; t < 4; t++) { b.user(`after ${String(t)}`); b.say(`after answer ${String(t)}`) }
    deliver(asPaged(b.recs), rng(14))
    expectCli(b.recs)
  })

  it('a non-prompt fork (hook attachment) is not a rewind: the conversation stays', () => {
    const b = conversation(2, 'rw-hook')
    const parent = b.parent!
    b.user('real follow-up')
    b.say('real answer')
    // A hook attachment written under an earlier assistant row (CLI 2.1.83 fork).
    b.recs.push({ type: 'attachment', uuid: 'rw-hook-att', parentUuid: parent, timestamp: '2026-01-01T10:00:00.000Z',
      _pos: 999_999, attachment: { type: 'hook_success', content: '' } } as unknown as Rec)
    deliver(asPaged(b.recs))
    expect(chatView(TAB).all.slice(-2)).toEqual(['U:real follow-up', 'A:real answer'])
  })
})

function conversationInto(b: TranscriptBuilder, turns: number, label: string): void {
  for (let t = 0; t < turns; t++) { b.user(`${label} question ${String(t)}`); b.say(`${label} answer ${String(t)}`) }
}
