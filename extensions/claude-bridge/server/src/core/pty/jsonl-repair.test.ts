import { afterEach, describe, expect, it, vi } from 'vitest'
import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import fsp from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { repairJsonlTail } from './jsonl-repair'
import { readRange } from './jsonl-lines'

vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn() }))
vi.mock('./jsonl-lines', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./jsonl-lines')>()
  return { ...actual, readRange: vi.fn(actual.readRange) }
})

const dirs: string[] = []
afterEach(() => {
  vi.restoreAllMocks()
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
})

const line = (o: unknown): string => JSON.stringify(o) + '\n'
const user = (uuid: string, parentUuid: string | null): string =>
  line({ type: 'user', uuid, parentUuid, message: { role: 'user', content: 'hi' } })
const done = (uuid: string, parentUuid: string | null, id = `msg_${uuid}`): string =>
  line({ type: 'assistant', uuid, parentUuid, message: { id, model: 'm', stop_reason: 'end_turn', content: [] } })
const interrupted = (uuid: string, parentUuid: string | null): string =>
  line({ type: 'assistant', uuid, parentUuid, message: { id: `msg_${uuid}`, model: 'm', stop_reason: null } })
const synthetic = (uuid: string, parentUuid: string | null): string =>
  line({
    type: 'assistant',
    uuid,
    parentUuid,
    message: { id: '0b7f3f5e-1111-4222-8333-444455556666', model: '<synthetic>', stop_reason: 'stop_sequence' },
  })

function setup(content: string): { dir: string; file: string } {
  const dir = mkdtempSync(join(tmpdir(), 'jsonl-repair-'))
  dirs.push(dir)
  const file = join(dir, 'c.jsonl')
  writeFileSync(file, content)
  return { dir, file }
}

/** A large, healthy prefix: the repair must never need to read it. */
function prefix(records: number): string {
  let s = ''
  let parent: string | null = null
  for (let i = 0; i < records; i++) {
    const u = `p${i}`
    s += i % 2 ? done(u, parent) : user(u, parent)
    parent = u
  }
  return s
}

describe('repairJsonlTail', () => {
  it('leaves a healthy transcript untouched and writes nothing', async () => {
    const { dir, file } = setup(prefix(50))
    const before = readFileSync(file)
    const stats = await repairJsonlTail(file)
    expect(stats.repaired).toBe(false)
    expect(readFileSync(file)).toEqual(before)
    expect(readdirSync(dir)).toEqual(['c.jsonl'])
  })

  it('drops a torn last line without touching the prefix', async () => {
    const head = prefix(2000)
    const { dir, file } = setup(head + '{"type":"user","uuid":"torn","parentUuid":"p1999","mess')
    const stats = await repairJsonlTail(file)
    expect(stats).toMatchObject({ repaired: true, droppedUnparseable: 1 })
    expect(readFileSync(file, 'utf8')).toBe(head)
    // Backup holds exactly the replaced bytes, not a copy of the whole file.
    const bak = readdirSync(dir).find((f) => f.includes('.bak-'))!
    expect(readFileSync(join(dir, bak), 'utf8')).toContain('"uuid":"torn"')
    expect(statSync(join(dir, bak)).size).toBeLessThan(200)
  })

  it('drops synthetic placeholders and re-parents their children to the kept chain', async () => {
    const head = prefix(10) // ends with p9 (completed assistant)
    const { file } = setup(head + user('u1', 'p9') + synthetic('s1', 'u1') + user('u2', 's1') + done('a2', 'u2'))
    const stats = await repairJsonlTail(file)
    expect(stats).toMatchObject({ repaired: true, droppedSynthetic: 1, relinkedParents: 1 })
    const rows = readFileSync(file, 'utf8')
      .trim()
      .split('\n')
      .map((l) => JSON.parse(l))
    expect(rows.some((r) => r.uuid === 's1')).toBe(false)
    expect(rows.find((r) => r.uuid === 'u2').parentUuid).toBe('u1')
    expect(readFileSync(file, 'utf8').startsWith(head)).toBe(true)
  })

  it('truncates a broken assistant tail back to the last completed turn', async () => {
    const head = prefix(10)
    const { file } = setup(head + user('u1', 'p9') + interrupted('a1', 'u1') + user('u2', 'a1'))
    const stats = await repairJsonlTail(file)
    expect(stats).toMatchObject({ repaired: true, truncatedTail: 3 })
    expect(readFileSync(file, 'utf8')).toBe(head)
  })

  it('never truncates across a compaction checkpoint', async () => {
    const head = prefix(10)
    const boundary = line({ type: 'system', subtype: 'compact_boundary', uuid: 'b', parentUuid: null })
    const { file } = setup(head + boundary + user('u1', 'b') + interrupted('a1', 'u1'))
    const stats = await repairJsonlTail(file)
    expect(stats.truncatedTail).toBe(2)
    expect(readFileSync(file, 'utf8')).toBe(head + boundary)
  })

  it('grows the window when the truncation anchor is older than it', async () => {
    const head = prefix(10)
    // A 300 KiB broken tail: no completed assistant inside a 64 KiB window.
    let tail = user('u1', 'p9') + interrupted('a1', 'u1')
    for (let i = 0; i < 300; i++) tail += user(`x${i}`, 'a1').replace('"hi"', JSON.stringify('y'.repeat(1000)))
    const { file } = setup(head + tail)
    const stats = await repairJsonlTail(file, { windowBytes: 64 * 1024, maxWindowBytes: 4 * 1024 * 1024 })
    expect(stats.repaired).toBe(true)
    expect(readFileSync(file, 'utf8')).toBe(head)
  })

  it('reads a bounded window of a large transcript, not the whole file', async () => {
    const head = prefix(40_000) // several MiB
    const { file } = setup(head + '{"torn":')
    const read = vi.mocked(readRange)
    read.mockClear()
    const stats = await repairJsonlTail(file, { windowBytes: 256 * 1024 })
    expect(stats.droppedUnparseable).toBe(1)
    expect(readFileSync(file, 'utf8')).toBe(head)
    const bytesRead = read.mock.calls.reduce((n, [, a, b]) => n + (b - a), 0)
    expect(statSync(file).size).toBeGreaterThan(4 * 1024 * 1024)
    expect(bytesRead).toBeLessThanOrEqual(256 * 1024)
  })

  it('still drops a torn line when no anchor fits the largest window', async () => {
    let body = user('u0', null)
    for (let i = 1; i < 200; i++) body += user(`u${i}`, `u${i - 1}`).replace('"hi"', JSON.stringify('z'.repeat(1000)))
    const { file } = setup(body + '{"torn":')
    const stats = await repairJsonlTail(file, { windowBytes: 16 * 1024, maxWindowBytes: 64 * 1024 })
    expect(stats).toMatchObject({ repaired: true, droppedUnparseable: 1, truncatedTail: 0 })
    expect(readFileSync(file, 'utf8')).toBe(body)
  })

  it('skips the rewrite when another writer appended during the analysis', async () => {
    const head = prefix(10)
    const { dir, file } = setup(head + '{"torn":')
    const realStat = fsp.stat
    let calls = 0
    vi.spyOn(fsp, 'stat').mockImplementation(async (...args: Parameters<typeof fsp.stat>) => {
      const st = await realStat(...args)
      // After the first stat (the analysis snapshot), a writer appends.
      if (++calls === 1) await fsp.appendFile(file, '\n' + user('late', 'p9'))
      return st
    })
    const stats = await repairJsonlTail(file)
    expect(stats.repaired).toBe(false)
    expect(readFileSync(file, 'utf8')).toContain('"uuid":"late"')
    expect(readdirSync(dir).filter((f) => f.includes('.bak-'))).toEqual([])
  })
})
