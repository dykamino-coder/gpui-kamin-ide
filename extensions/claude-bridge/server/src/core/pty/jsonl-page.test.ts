import { afterEach, describe, expect, it } from 'vitest'
import { mkdtemp, open, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { HISTORY_PAGE_BYTES, HISTORY_PAGE_RECORDS, readHistoryPage } from './jsonl-page'
import { SegmentIndexBuilder } from './segment-index'

const dirs: string[] = []
afterEach(async () => {
  for (const dir of dirs.splice(0)) await rm(dir, { recursive: true, force: true })
})
async function fixture() {
  const dir = await mkdtemp(join(tmpdir(), 'history-page-'))
  dirs.push(dir)
  return join(dir, 'conversation.jsonl')
}
const line = (row: unknown) => `${JSON.stringify(row)}\n`

describe('bounded history pages', () => {
  it('paints a 256 MiB transcript tail without reading or transmitting its prefix', async () => {
    const file = await fixture()
    const fh = await open(file, 'w')
    const block = Buffer.from(
      Array.from({ length: 256 }, (_, i) =>
        line({ type: 'user', uuid: `prefix-${i}`, message: { content: 'x'.repeat(4000) } }),
      ).join(''),
    )
    let size = 0
    try {
      while (size < 256 * 1024 * 1024) {
        await fh.write(block)
        size += block.length
      }
      const tail = Array.from({ length: 600 }, (_, i) =>
        line({ type: 'assistant', uuid: `tail-${i}`, message: { content: 'recent' } }),
      ).join('')
      await fh.write(tail)
      size += Buffer.byteLength(tail)
    } finally {
      await fh.close()
    }
    const began = performance.now()
    const page = await readHistoryPage(file, Infinity)
    expect(performance.now() - began).toBeLessThan(2000)
    expect(page.bytesRead).toBeLessThanOrEqual(HISTORY_PAGE_BYTES)
    expect(page.records).toHaveLength(HISTORY_PAGE_RECORDS)
    expect(page.records[0]?.uuid).toBe('tail-400')
    expect(page.records.at(-1)?.uuid).toBe('tail-599')
    expect(page.endPos).toBe(size)
    const older = await readHistoryPage(file, page.nextPos)
    expect(older.records[0]?.uuid).toBe('tail-200')
    expect(older.records.at(-1)?.uuid).toBe('tail-399')
  }, 15000)

  it('keeps every idle-compaction tail row on its physical side of the boundary', async () => {
    const file = await fixture()
    const rows = [
      { type: 'system', subtype: 'compact_boundary', uuid: 'b1', timestamp: '2026-10-27T14:32:00Z' },
      { type: 'assistant', uuid: 'wrong-session', timestamp: '2026-10-29T13:00:00Z' },
      { type: 'assistant', uuid: 'waiting', timestamp: '2026-10-29T14:55:59Z' },
      {
        type: 'user',
        uuid: 'notification',
        isMeta: true,
        message: { content: '<task-notification>completed</task-notification>' },
      },
      {
        type: 'assistant',
        uuid: 'final-table',
        parentUuid: 'notification',
        timestamp: '2026-10-29T14:56:30Z',
        message: { content: '0.7 MB instead of 5.4' },
      },
      { type: 'system', subtype: 'turn_duration', uuid: 'duration', durationMs: 37000 },
      {
        type: 'system',
        subtype: 'compact_boundary',
        uuid: 'b2',
        logicalParentUuid: 'final-table',
        timestamp: '2026-10-29T14:56:00Z',
        compactMetadata: { trigger: 'auto' },
      },
      { type: 'user', isCompactSummary: true, uuid: 'summary' },
      { type: 'assistant', uuid: 'current' },
    ]
    const builder = new SegmentIndexBuilder()
    const positions: number[] = []
    let pos = 0
    for (const row of rows) {
      positions.push(pos)
      builder.add(row as never, pos)
      pos += Buffer.byteLength(line(row))
    }
    await writeFile(file, rows.map(line).join(''))
    const boundaries = builder.build().boundaries
    expect(boundaries.map((b) => b.pos)).toEqual([positions[0], positions[6]])
    const archive = await readHistoryPage(file, boundaries[1]!.pos!, boundaries[0]!.pos!)
    expect(archive.records.map((r) => r.uuid)).toEqual(rows.slice(0, 6).map((r) => r.uuid))
    const current = await readHistoryPage(file, Infinity, boundaries[1]!.pos!)
    expect(current.records.map((r) => r.uuid)).toEqual(['b2', 'summary', 'current'])
  })

  it('does not advance past an unfinished live row and reads oversized rows whole', async () => {
    const file = await fixture()
    const first = line({ type: 'user', uuid: 'first' })
    const huge = line({ type: 'assistant', uuid: 'huge', message: { content: 'я'.repeat(HISTORY_PAGE_BYTES) } })
    await writeFile(file, first + huge + '{"type":"assistant"')
    const page = await readHistoryPage(file, Infinity)
    expect(page.records.at(-1)?.uuid).toBe('huge')
    expect(page.endPos).toBe(Buffer.byteLength(first + huge))
  })
})
