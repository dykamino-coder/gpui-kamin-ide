// Ordering test for JsonlBatcher.
//
// PROVES the contract `replayJsonlToRenderer` depends on: after `flushMain()`
// returns, NOTHING the batcher was holding may still land on the renderer. It
// flushes and then immediately sends its own full snapshot on the same channel,
// so a late chunk arriving afterwards means overlapping entries → dedup and
// scroll-pin confusion (the failure its own comment says the flush prevents).
//
// The regression this pins: `flushMain()` used to no-op while a drain was in
// flight, so a reload landing mid-drain (>150 entries = multi-chunk) let the
// leftovers trickle out AFTER the snapshot.
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>
// Pure logic — the electron BrowserWindow import is type-only, so tsx erases it.

import { JsonlBatcher } from './jsonl-batcher.ts'

let failures = 0
function assert(cond: boolean, msg: string): void {
  if (!cond) { failures++; console.error('  ✗ ' + msg) }
}

const BATCH_MS = 50
const sleep = (ms: number): Promise<void> => new Promise(r => setTimeout(r, ms))

/** Records every send in order. `entries:<n>` for a chunk, or the raw channel.
 *  `firstChunk` resolves as the FIRST chunk goes out — awaiting it lands us
 *  deterministically mid-drain: the continuation is a microtask, and a microtask
 *  always runs before the drain's own setTimeout(0) yield. (Sleeping instead is
 *  a race — 0ms yields push 1–3 chunks through in the same millisecond, which is
 *  what made an earlier version of this test flaky.) */
function fakeWindow() {
  const log: string[] = []
  let announce: (() => void) | null = null
  const firstChunk = new Promise<void>((resolve) => { announce = resolve })
  const win = {
    isDestroyed: () => false,
    webContents: {
      send: (channel: string, _tabId: string, payload: unknown) => {
        log.push(channel === 'jsonl-entries' ? `entries:${String((payload as unknown[]).length)}` : channel)
        if (channel === 'jsonl-entries' && announce) { announce(); announce = null }
      },
    },
  }
  return { win, log, firstChunk }
}
const mk = (n: number, from = 0): unknown[] => Array.from({ length: n }, (_, i) => ({ uuid: `u${String(from + i)}` }))

// ── Scenario 1: flushMain mid-drain must put EVERYTHING out before the caller's
// next send (the exact replay-after-reload sequence).
console.log('Scenario 1: flushMain() mid-drain beats the replay snapshot')
{
  const { win, log, firstChunk } = fakeWindow()
  const b = new JsonlBatcher(win as never, 'tab1')
  b.queueMain(mk(400))            // 3 chunks: 150 + 150 + 100
  await firstChunk                // exactly mid-drain: chunk 1 out, 250 pending
  assert(log.length === 1, `S1 poised mid-drain, one chunk out (got ${String(log.length)})`)

  b.flushMain()                                        // ← reload path
  win.webContents.send('jsonl-status', 'tab1', [])     // ← replay snapshot, right after
  const afterSnapshot = log.slice(log.indexOf('jsonl-status') + 1)
  assert(log.indexOf('jsonl-status') !== -1, 'S1 snapshot was sent')
  assert(afterSnapshot.length === 0, `S1 NOTHING may land after the snapshot (late: ${afterSnapshot.join(',')})`)

  await sleep(30) // let any surviving drain timer fire — it must have none
  const late = log.slice(log.indexOf('jsonl-status') + 1)
  assert(late.length === 0, `S1 no late chunk after the drain timer would have fired (late: ${late.join(',')})`)
  assert(log.join(',') === 'entries:150,entries:150,entries:100,jsonl-status',
    `S1 exact order+chunking preserved (got: ${log.join(',')})`)
}

// ── Scenario 2: the frame cap survives the synchronous flush (no giant IPC).
console.log('Scenario 2: flushMain still chunks — never one huge send')
{
  const { win, log } = fakeWindow()
  const b = new JsonlBatcher(win as never, 'tab1')
  b.queueMain(mk(1000))
  b.flushMain()
  assert(log.every(l => l.startsWith('entries:')), 'S2 only entry chunks')
  assert(log.every(l => Number(l.split(':')[1]) <= 150), `S2 every chunk ≤150 (got: ${log.join(',')})`)
  assert(log.length === 7, `S2 1000 entries → 7 chunks (got ${String(log.length)})`)
}

// ── Scenario 3: entries queued DURING a drain still ride out, in order.
console.log('Scenario 3: append during drain is not lost')
{
  const { win, log, firstChunk } = fakeWindow()
  const b = new JsonlBatcher(win as never, 'tab1')
  b.queueMain(mk(200))
  await firstChunk             // chunk 1 out (150), 50 left, drain in flight
  b.queueMain(mk(10, 200))     // late arrival mid-drain
  await sleep(40)              // let the drain finish on its own
  const total = log.filter(l => l.startsWith('entries:')).reduce((s, l) => s + Number(l.split(':')[1]), 0)
  assert(total === 210, `S3 all 210 entries delivered (got ${String(total)})`)
}

// ── Scenario 4: clear() must kill an in-flight drain (compaction path).
console.log('Scenario 4: clear() cancels the drain')
{
  const { win, log, firstChunk } = fakeWindow()
  const b = new JsonlBatcher(win as never, 'tab1')
  b.queueMain(mk(400))
  await firstChunk             // drain in flight, 250 pending
  const before = log.length
  b.clear()                    // compaction: pre-compact entries must NOT leak
  await sleep(40)
  assert(log.length === before, `S4 no chunk escapes after clear() (got ${String(log.length - before)} extra)`)
  // and the batcher is still usable afterwards
  b.queueMain(mk(5, 900))
  await sleep(BATCH_MS + 10)
  assert(log.length === before + 1, 'S4 batcher still flushes new entries after clear()')
}

// ── Scenario 5: drainMainThen — the replay contract ────────────────────────
// The whole transcript used to go out in ONE ipc (46MB frame / 191ms of parse;
// ~169MB at the cache cap). Now it chunks AND yields — which means the caller's
// trailing sends (jsonl-status/replayComplete, tree, streaming stub) must ride
// the continuation, or they overtake the chunks still in flight and the renderer
// sees "replay done" before the history it completes.
console.log('Scenario 5: drainMainThen puts the tail AFTER every chunk')
{
  const { win, log } = fakeWindow()
  const b = new JsonlBatcher(win as never, 'tab1')
  b.drainMainThen(mk(400), () => { win.webContents.send('jsonl-status', 'tab1', []) })
  await sleep(60)
  assert(log.join(',') === 'entries:150,entries:150,entries:100,jsonl-status',
    `S5 chunks then tail, in order (got: ${log.join(',')})`)
  assert(log.filter(l => l === 'jsonl-status').length === 1, 'S5 the tail runs exactly once')
}

// ── Scenario 6: it YIELDS (that's the entire point) ─────────────────────────
console.log('Scenario 6: drainMainThen yields between chunks')
{
  const { win, log } = fakeWindow()
  const b = new JsonlBatcher(win as never, 'tab1')
  b.drainMainThen(mk(400), () => { /* no tail */ })
  // Synchronously after the call only the FIRST chunk may be out — if all three
  // are, we rebuilt the very stall this replaces.
  assert(log.length === 1, `S6 only one chunk sent synchronously (got ${String(log.length)})`)
  await sleep(60)
  assert(log.length === 3, `S6 the rest drain across later turns (got ${String(log.length)})`)
}

// ── Scenario 7: a second replay mid-drain must not orphan the first tail ───
// useInit fires two replay-triggering messages back to back on every cold start
// with live tabs, so this is reachable, not theoretical.
console.log('Scenario 7: re-entrant replay settles the first continuation')
{
  const { win, log, firstChunk } = fakeWindow()
  const b = new JsonlBatcher(win as never, 'tab1')
  let firstTail = 0, secondTail = 0
  b.drainMainThen(mk(400), () => { firstTail++; win.webContents.send('tail-1', 'tab1', []) })
  await firstChunk
  b.drainMainThen(mk(200), () => { secondTail++; win.webContents.send('tail-2', 'tab1', []) })
  await sleep(80)
  assert(firstTail === 1, `S7 the FIRST tail still ran (got ${String(firstTail)}) — a dropped one strands the renderer in replay mode`)
  assert(secondTail === 1, `S7 the second tail ran once (got ${String(secondTail)})`)
  assert(log.indexOf('tail-1') < log.indexOf('tail-2'), 'S7 …and in order')
  assert(log.lastIndexOf('entries:150') < log.indexOf('tail-2') || log.indexOf('tail-2') === log.length - 1,
    'S7 the second replay chunks precede its own tail')
}

console.log('')
if (failures === 0) console.log('✅ ALL BATCHER ORDERING CHECKS PASSED')
else { console.error(`❌ ${failures} check(s) FAILED`); process.exit(1) }
