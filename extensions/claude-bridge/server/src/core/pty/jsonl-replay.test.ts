import { afterEach, describe, expect, it } from 'vitest'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'
import { buildSegmentIndex } from './segment-index'
import { REPLAY_BATCH_BYTES, type SendOutcome } from './jsonl-replay'
import type { JsonlEntry, JsonlStatus } from '../../shared/jsonl-types'

type Tagged = JsonlEntry & { _pos?: number; _posEnd?: number; _ord?: number; _oversized?: number }

const dirs: string[] = []
const watchers: JsonlWatcher[] = []
afterEach(() => {
  for (const w of watchers.splice(0)) w.stop()
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
})

const row = (i: number, text: string, extra: Record<string, unknown> = {}): Record<string, unknown> => ({
  type: i % 2 ? 'assistant' : 'user',
  uuid: `u-${String(i)}`,
  timestamp: new Date(Date.UTC(2026, 0, 1, 0, 0, i)).toISOString(),
  message: { role: i % 2 ? 'assistant' : 'user', content: text },
  ...extra,
})

function transcript(rows: Record<string, unknown>[]): string {
  const dir = mkdtempSync(join(tmpdir(), 'jsonl-replay-'))
  dirs.push(dir)
  const file = join(dir, 't.jsonl')
  writeFileSync(file, rows.map((r) => JSON.stringify(r)).join('\n') + '\n')
  return file
}

interface Harness {
  w: JsonlWatcher
  frames: { entries: Tagged[]; replay: boolean; bytes: number }[]
  statuses: JsonlStatus[]
  stats: string[]
  done: () => Promise<void>
}

function harness(
  file: string,
  send: (entries: Tagged[], bytes: number) => SendOutcome | boolean = () => 'sent',
  writable?: () => boolean,
): Harness {
  const frames: Harness['frames'] = []
  const statuses: JsonlStatus[] = []
  const stats: string[] = []
  const w = new JsonlWatcher(
    Date.now(),
    (entries, replay) => {
      const bytes = Buffer.byteLength(JSON.stringify(entries))
      const r = send(entries as Tagged[], bytes)
      if (r === 'sent' || r === true) frames.push({ entries: entries as Tagged[], replay: !!replay, bytes })
      return r
    },
    (s) => statuses.push(s),
    undefined,
    undefined,
    undefined,
    undefined,
    () => {
      stats.length = 0
    },
    (e) => {
      stats.push(String(e.uuid))
    },
    writable,
  )
  watchers.push(w)
  ;(w as unknown as { filePath: string }).filePath = file
  const done = async (): Promise<void> => {
    await expect.poll(() => statuses.some((s) => s.replayComplete), { timeout: 15_000 }).toBe(true)
  }
  return { w, frames, statuses, stats, done }
}

describe('bounded replay pipeline', () => {
  it('cuts replay frames by source bytes, not by record count', async () => {
    // 200 × 100 KiB used to be ONE 20 MB frame, refused by the 16 MiB cap forever.
    const rows = Array.from({ length: 200 }, (_, i) => row(i, 'x'.repeat(100 * 1024)))
    const h = harness(transcript(rows), (_e, bytes) => (bytes > 16 * 1024 * 1024 ? 'oversized' : 'sent'))
    h.w.replayAll()
    await h.done()
    const replayed = h.frames.filter((f) => f.replay)
    const uuids = new Set(replayed.flatMap((f) => f.entries.map((e) => e.uuid)))
    expect(uuids.size).toBe(200)
    for (const f of replayed) expect(f.bytes).toBeLessThan(REPLAY_BATCH_BYTES + 256 * 1024)
  })

  it('delivers a record that can never fit as an explicit stub instead of retrying forever', async () => {
    const rows = [row(0, 'small'), row(1, 'g'.repeat(3 * 1024 * 1024)), row(2, 'after')]
    let attempts = 0
    const h = harness(transcript(rows), (_e, bytes) => {
      attempts++
      return bytes > 1024 * 1024 ? 'oversized' : 'sent'
    })
    h.w.replayAll()
    await h.done()
    const all = h.frames.flatMap((f) => f.entries)
    const stub = all.find((e) => e.uuid === 'u-1' && e._oversized)
    expect(stub).toBeDefined()
    expect(stub!._posEnd! - stub!._pos!).toBeGreaterThan(3 * 1024 * 1024)
    expect(all.some((e) => e.uuid === 'u-2')).toBe(true)
    expect(attempts).toBeLessThan(20)
  })

  it('waits for the socket to drain instead of re-sending, and loses nothing', async () => {
    const rows = Array.from({ length: 1000 }, (_, i) => row(i, 'y'.repeat(2000)))
    let open = false
    let sendsWhileBlocked = 0
    const h = harness(
      transcript(rows),
      () => {
        if (!open) sendsWhileBlocked++
        return 'sent'
      },
      () => open,
    )
    h.w.replayAll()
    await new Promise((r) => setTimeout(r, 300))
    expect(sendsWhileBlocked).toBe(0)
    expect(h.frames.length).toBe(0)
    open = true
    await h.done()
    const uuids = new Set(h.frames.flatMap((f) => f.entries.map((e) => e.uuid)))
    expect(uuids.size).toBe(1000)
  })

  it('stop() ends the replay: no frame and no stats after cancellation', async () => {
    const rows = Array.from({ length: 4000 }, (_, i) => row(i, 'z'.repeat(1000)))
    let gate = false
    const h = harness(
      transcript(rows),
      () => 'sent',
      () => gate,
    )
    h.w.replayAll()
    await expect.poll(() => h.stats.length, { timeout: 5000 }).toBe(4000)
    h.w.stop()
    gate = true
    const framesAtStop = h.frames.length
    await new Promise((r) => setTimeout(r, 300))
    expect(h.frames.length).toBe(framesAtStop)
    expect(h.statuses.some((s) => s.replayComplete)).toBe(false)
  })

  it('keeps _ord file order, exact offsets, stats order and the segment index', async () => {
    const rows: Record<string, unknown>[] = []
    for (let i = 0; i < 700; i++) {
      if (i === 250 || i === 600) {
        rows.push({
          type: 'system',
          subtype: 'compact_boundary',
          uuid: `b-${i}`,
          timestamp: `2026-02-0${i === 250 ? 1 : 2}T00:00:00Z`,
        })
      }
      rows.push(row(i, `t${i} ` + 'é😀'.repeat(50)))
    }
    const file = transcript(rows)
    const h = harness(file)
    h.w.replayAll()
    await h.done()
    const replayed = h.frames.filter((f) => f.replay).flatMap((f) => f.entries)
    const byUuid = new Map(replayed.map((e) => [e.uuid, e]))
    expect(byUuid.size).toBe(rows.length)
    const ordered = [...byUuid.values()].sort((a, b) => a._ord! - b._ord!)
    expect(ordered.map((e) => e.uuid)).toEqual(rows.map((r) => r.uuid))
    const bytes = readFileSync(file)
    for (const e of ordered.slice(0, 50)) {
      expect(JSON.parse(bytes.toString('utf8', e._pos!, e._posEnd! - 1)).uuid).toBe(e.uuid)
    }
    expect(h.stats).toEqual(rows.map((r) => r.uuid))
    const bounds = rows.flatMap((r, i) => (r.subtype === 'compact_boundary' ? [i] : []))
    const final = h.statuses.filter((s) => s.replayComplete).at(-1)!
    expect(final.segmentIndex).toEqual(buildSegmentIndex(rows as unknown as JsonlEntry[], bounds))
  })

  it('records user inputs only for live appends, never for replayed history', async () => {
    const rows = Array.from({ length: 40 }, (_, i) => row(i, `m${i}`))
    const file = transcript(rows)
    const inputs: string[] = []
    const statuses: JsonlStatus[] = []
    const w = new JsonlWatcher(
      Date.now(),
      () => 'sent',
      (s) => statuses.push(s),
      (e) => inputs.push(String(e.uuid)),
    )
    watchers.push(w)
    ;(w as unknown as { filePath: string }).filePath = file
    w.replayAll()
    await expect.poll(() => statuses.some((s) => s.replayComplete), { timeout: 5000 }).toBe(true)
    expect(inputs).toEqual([])
    writeFileSync(file, JSON.stringify(row(100, 'live')) + '\n', { flag: 'a' })
    await (w as unknown as { checkForNewContent(): Promise<void> }).checkForNewContent()
    expect(inputs).toEqual(['u-100'])
  })

  it('streamFrom resumes exactly at a byte boundary in byte-bounded batches', async () => {
    const rows = Array.from({ length: 300 }, (_, i) => row(i, 'q'.repeat(10_000)))
    const file = transcript(rows)
    const h = harness(file)
    const lines = rows.map((r) => Buffer.byteLength(JSON.stringify(r)) + 1)
    const from = lines.slice(0, 100).reduce((a, b) => a + b, 0)
    const end = await h.w.streamFrom(from, () => true)
    expect(end).toBe(lines.reduce((a, b) => a + b, 0))
    const got = h.frames.flatMap((f) => f.entries.map((e) => e.uuid))
    expect(got).toEqual(rows.slice(100).map((r) => r.uuid))
    for (const f of h.frames) expect(f.bytes).toBeLessThan(REPLAY_BATCH_BYTES + 256 * 1024)
  })
})
