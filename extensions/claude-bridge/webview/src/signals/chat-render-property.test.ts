// Property test for the chat-render audit (extensions/claude-bridge/docs/chat-pipeline.md §7).
//
// For many random synthetic transcripts and random delivery schedules — live or
// paged rows, random chunking, duplicate and out-of-order batches, trims and
// store clears at random points — the chat must end up rendering exactly the
// messages the CLI shows, once each, in CLI order, after the refill the viewer
// requests. Mid-run it must never render a message twice.
//
// Rewinds are included: the CLI shows only the branch it resumes (D4).
import { describe, it, expect } from 'vitest'
import { appendJsonlEntries, clearJsonlEntries, trimTabToWindow, getJsonlEntries, tabNeedsRefill, clearRefillDebt } from './jsonl'
import { activeTabId } from './tabs'
import { TranscriptBuilder, asPaged, asLive, rng, type Rec } from './chat-audit/transcript'
import { cliView, chatView, isSuffix } from './chat-audit/views'
import type { JsonlEntryData } from '../types/jsonl'

function randomTranscript(seed: number, rand: () => number): TranscriptBuilder {
  const b = new TranscriptBuilder(`p${String(seed)}`)
  const turns = 8 + Math.floor(rand() * 40)
  let compactions = 0
  const forks: (string | undefined)[] = []
  for (let t = 0; t < turns; t++) {
    forks.push(b.parent)
    if (t > 2 && rand() < 0.06) b.rewindTo(forks[Math.floor(rand() * forks.length)]!)
    b.user(`prompt ${String(seed)}.${String(t)}`)
    const k = rand()
    if (k < 0.3) b.say(`reply ${String(seed)}.${String(t)}`)
    else if (k < 0.45) b.assistant([{ type: 'thinking', thinking: '' }, { type: 'text', text: `a${String(t)}` }, { type: 'text', text: `b${String(t)}` }])
    else if (k < 0.6) { b.toolBurst(`tools ${String(t)}`, 1 + Math.floor(rand() * 3)); b.say(`after tools ${String(t)}`) }
    else if (k < 0.68) b.interrupted(`partial ${String(t)}`)
    else if (k < 0.76) { b.meta(`<system-reminder>r${String(t)}</system-reminder>`); b.say(`after meta ${String(t)}`) }
    else if (k < 0.84) { b.bookkeeping(1 + Math.floor(rand() * 20), rand() < 0.5 ? 'cost-state' : 'api-request'); b.say(`after bookkeeping ${String(t)}`) }
    else if (k < 0.9) { b.sidechain(`sub ${String(t)}`); b.say(`after sub ${String(t)}`) }
    else if (k < 0.96) { b.queueOperation('enqueue', `q${String(t)}`); b.say(`busy ${String(t)}`); b.queueOperation('dequeue', `q${String(t)}`) }
    // A rewind only reaches back inside the current conversation.
    else if (compactions < 2) { compactions++; b.compact(`summary ${String(t)}`); forks.length = 0 }
    else b.say(`reply ${String(seed)}.${String(t)}`)
  }
  return b
}

function duplicates(lines: string[]): string[] {
  const seen = new Set<string>()
  return lines.filter((l) => (seen.has(l) ? true : (seen.add(l), false)))
}

function run(seed: number): void {
  const rand = rng(seed)
  const b = randomTranscript(seed, rand)
  const tab = `prop-${String(seed)}`
  activeTabId.value = tab
  clearJsonlEntries(tab)
  clearRefillDebt(tab)
  const cli = cliView(b.recs)
  let owed = false
  let held: JsonlEntryData[] | null = null
  for (let i = 0; i < b.recs.length;) {
    const r = rand()
    if (r < 0.65) {
      const n = 1 + Math.floor(rand() * 30)
      const slice: Rec[] = b.recs.slice(i, i + n)
      i += n
      const rows = rand() < 0.5 ? asLive(slice) : asPaged(slice)
      if (rand() < 0.15 && !held) { held = rows; continue } // arrives after the next batch
      appendJsonlEntries(tab, rows)
      if (rand() < 0.15) appendJsonlEntries(tab, structuredClone(rows)) // duplicate delivery
      if (held) { appendJsonlEntries(tab, held); held = null }
    } else if (r < 0.75) {
      const keep = 5 + Math.floor(rand() * 60)
      if (trimTabToWindow(tab, keep)) owed = true // bottom trim; scroll-up/replay owes the rest
    } else if (r < 0.8) {
      clearJsonlEntries(tab)
      owed = true
    }
    expect(duplicates(chatView(tab).all), `seed ${String(seed)}: duplicate mid-run`).toEqual([])
  }
  if (held) appendJsonlEntries(tab, held)
  const before = chatView(tab)
  if (!owed) expect(before.all, `seed ${String(seed)}: no refill needed`).toEqual(cli)
  else expect(isSuffix(before.window, cli) || getJsonlEntries(tab).length === 0 || tabNeedsRefill(tab)).toBe(true)
  // The refill the viewer requests (replay on activation / reattach): the
  // whole transcript again, newest batch first, with fresh replay ords.
  const all = asLive(b.recs).map((e, k) => ({ ...e, _ord: 10_000_000 + k }))
  for (let end = all.length; end > 0; end -= 37) appendJsonlEntries(tab, all.slice(Math.max(0, end - 37), end))
  expect(chatView(tab).all, `seed ${String(seed)}: after refill`).toEqual(cli)
}

describe('chat renders the CLI view exactly once under any delivery schedule', () => {
  for (let seed = 1; seed <= 60; seed++) it(`seed ${String(seed)}`, () => { run(seed) })
})
