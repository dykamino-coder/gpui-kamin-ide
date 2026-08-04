// readRange is the foundation for scroll-up: the webview holds only a recent
// window and pulls the preceding page from the disk mirror on demand, keyed on
// the stable byte offset `_pos` (never `_ord`, which the server re-tags). These
// build a real mirror file and read older pages back out of it.
import { mkdtemp, rm } from 'fs/promises'
import { tmpdir } from 'os'
import { join } from 'path'
import { describe, it, expect, afterAll } from 'vitest'
import { TranscriptMirror } from './transcript-mirror'

const dirs: string[] = []
afterAll(async () => { for (const d of dirs) await rm(d, { recursive: true, force: true }) })

// Build `n` records and write them through append, returning the REAL byte
// offset each record lands at. `_pos` is set before serialising and nothing is
// mutated afterwards, so the measured length matches exactly what append writes
// (the earlier off-by-one was mutating `_posEnd` after measuring the length).
async function realMirror(n: number): Promise<{ m: TranscriptMirror; posOf: number[] }> {
  const dir = await mkdtemp(join(tmpdir(), 'mir-'))
  dirs.push(dir)
  const m = new TranscriptMirror(dir, 'conv')
  const posOf: number[] = []
  const recs: { _pos: number; uuid: string; type: string }[] = []
  let p = 0
  for (let i = 0; i < n; i++) {
    const rec = { _pos: p, uuid: `u${i}`, type: 'user' }
    const len = Buffer.byteLength(JSON.stringify(rec), 'utf8') + 1 // + newline
    posOf.push(p)
    recs.push(rec)
    p += len
  }
  await m.append(recs, 'head')
  return { m, posOf }
}

describe('TranscriptMirror.readRange', () => {
  it('returns nothing for a degenerate request', async () => {
    const { m } = await realMirror(5)
    expect(await m.readRange(0, 10)).toEqual([])
    expect(await m.readRange(1000, 0)).toEqual([])
  })

  it('returns the page immediately before the boundary, oldest→newest', async () => {
    const { m, posOf } = await realMirror(50)
    const page = (await m.readRange(posOf[20]!, 10)) as { uuid: string; _pos: number }[]
    expect(page).toHaveLength(10)
    // The 10 closest below #20 are #10..#19, ascending.
    expect(page.map((r) => r.uuid)).toEqual(['u10', 'u11', 'u12', 'u13', 'u14', 'u15', 'u16', 'u17', 'u18', 'u19'])
    expect(page.every((r) => r._pos < posOf[20]!)).toBe(true)
  })

  it('never returns a record at or after the boundary (no overlap with the held window)', async () => {
    const { m, posOf } = await realMirror(40)
    const page = (await m.readRange(posOf[15]!, 100)) as { _pos: number }[]
    expect(page.every((r) => r._pos < posOf[15]!)).toBe(true)
    expect(Math.max(...page.map((r) => r._pos))).toBe(posOf[14]!) // the record just below the boundary
  })

  it('clamps to the start of the file when asked for more than exists', async () => {
    const { m, posOf } = await realMirror(8)
    const page = (await m.readRange(posOf[5]!, 100)) as { uuid: string }[]
    expect(page.map((r) => r.uuid)).toEqual(['u0', 'u1', 'u2', 'u3', 'u4'])
  })

  it('returns [] for a missing mirror file instead of throwing', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'mir-'))
    dirs.push(dir)
    const m = new TranscriptMirror(dir, 'does-not-exist')
    expect(await m.readRange(1000, 5)).toEqual([])
  })
})

describe('TranscriptMirror.rewriteInOrder', () => {
  // Fixes the resumed-session hole: replayed history arrives out of order and is
  // written down in one shot so the windowed store can scroll it back. Records
  // must land in FILE order (by _pos) or readRange's byte-offset seek is wrong.
  it('writes shuffled input in _pos order, and readRange then works', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'mir-'))
    dirs.push(dir)
    const m = new TranscriptMirror(dir, 'conv')
    // Build 30 records whose `_pos` equals their real byte offset in the written
    // file. No `_posEnd` field, so the serialised length is self-consistent (the
    // earlier off-by-one was a multi-digit `_posEnd` changing the line length).
    const recs: { _pos: number; uuid: string }[] = []
    let p = 0
    for (let i = 0; i < 30; i++) {
      const rec = { _pos: p, uuid: `u${i}` }
      recs.push(rec)
      p += Buffer.byteLength(JSON.stringify(rec), 'utf8') + 1
    }
    const shuffled = [...recs.slice(20), ...recs.slice(0, 20)] // newest block first
    await m.rewriteInOrder(shuffled, 'head')

    // Tail comes back oldest→newest (file order restored).
    const tail = (await m.readTail(5)) as { uuid: string }[]
    expect(tail.map((r) => r.uuid)).toEqual(['u25', 'u26', 'u27', 'u28', 'u29'])
    // And an older page reads correctly by byte offset.
    const page = (await m.readRange(recs[10]!._pos, 4)) as { uuid: string }[]
    expect(page.map((r) => r.uuid)).toEqual(['u6', 'u7', 'u8', 'u9'])
  })

  it('no-ops on empty input', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'mir-'))
    dirs.push(dir)
    const m = new TranscriptMirror(dir, 'conv')
    await m.rewriteInOrder([], 'head')
    expect(await m.readTail(5)).toEqual([])
  })

  it('NEVER shrinks an existing mirror — a lived-from-start >cache session keeps its full disk copy', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'mir-'))
    dirs.push(dir)
    const m = new TranscriptMirror(dir, 'conv')
    // The mirror already holds 40 live-appended records (the uncapped on-disk copy).
    const built: { _pos: number; uuid: string }[] = []
    let p = 0
    for (let i = 0; i < 40; i++) { const r = { _pos: p, uuid: `u${i}` }; built.push(r); p += Buffer.byteLength(JSON.stringify(r), 'utf8') + 1 }
    await m.append(built, 'head')
    // A reconnect fires rewriteInOrder with only the capped cache (the last 10).
    // It must NOT overwrite — that would truncate u0..u29 off the disk forever.
    await m.rewriteInOrder(built.slice(30), 'head')
    const tail = (await m.readTail(40)) as { uuid: string }[]
    expect(tail).toHaveLength(40) // full history intact, not shrunk to 10
    expect(tail[0]!.uuid).toBe('u0')
  })

  it('dedups by uuid when initialising an empty mirror', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'mir-'))
    dirs.push(dir)
    const m = new TranscriptMirror(dir, 'conv')
    await m.rewriteInOrder([{ _pos: 0, uuid: 'a' }, { _pos: 50, uuid: 'a' }, { _pos: 100, uuid: 'b' }], 'head')
    const all = (await m.readTail(10)) as { uuid: string }[]
    expect(all.map((r) => r.uuid)).toEqual(['a', 'b']) // 'a' not doubled
  })
})
