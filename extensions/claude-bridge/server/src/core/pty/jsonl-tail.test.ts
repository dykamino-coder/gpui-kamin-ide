import { afterEach, expect, it } from 'vitest'
import { appendFile, mkdtemp, rm, truncate, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { TranscriptTail } from './jsonl-tail'

const SKIP = '"type":"file-history-snapshot"'
const dirs: string[] = []
const tails: TranscriptTail[] = []
afterEach(async () => {
  for (const t of tails.splice(0)) t.close()
  for (const d of dirs.splice(0)) await rm(d, { recursive: true, force: true })
})
const rec = (uuid: string, extra: Record<string, unknown> = {}) =>
  `${JSON.stringify({ type: 'user', uuid, ...extra })}\n`

async function tailOf(content: string, batch = 4 * 1024 * 1024) {
  const dir = await mkdtemp(join(tmpdir(), 'tail-'))
  dirs.push(dir)
  const file = join(dir, 'c.jsonl')
  await writeFile(file, content)
  const tail = new TranscriptTail(file, SKIP, batch)
  tails.push(tail)
  return { file, tail }
}

it('reads complete records with exact byte spans and leaves a torn line for the next pass', async () => {
  const a = rec('a', { text: 'ünïcødé' })
  const { file, tail } = await tailOf(a + '{"type":"user","uuid":"b"')
  const first = await tail.read(0)
  expect(first.entries.map((e) => e.uuid)).toEqual(['a'])
  expect(first.spans).toEqual([[0, Buffer.byteLength(a)]])
  expect(first.endPos).toBe(Buffer.byteLength(a))
  expect((first.entries[0] as { _posEnd?: number })._posEnd).toBe(Buffer.byteLength(a))
  await appendFile(file, '}\n')
  const second = await tail.read(first.endPos)
  expect(second.entries.map((e) => e.uuid)).toEqual(['b'])
  expect(second.endPos).toBe(Buffer.byteLength(a) + Buffer.byteLength('{"type":"user","uuid":"b"}\n'))
})

it('skips marker, blank and malformed terminated lines but consumes their bytes', async () => {
  const content = `{"type":"file-history-snapshot","x":1}\n\n{broken}\n` + rec('ok')
  const { tail } = await tailOf(content)
  const pass = await tail.read(0)
  expect(pass.entries.map((e) => e.uuid)).toEqual(['ok'])
  expect(pass.endPos).toBe(Buffer.byteLength(content))
})

it('reads a record longer than the batch cap whole', async () => {
  const big = rec('big', { text: 'x'.repeat(300 * 1024) })
  const { tail } = await tailOf(big + rec('next'), 64 * 1024)
  const pass = await tail.read(0)
  expect(pass.entries[0]?.uuid).toBe('big')
  expect(pass.spans[0]).toEqual([0, Buffer.byteLength(big)])
})

it('reports the file metadata only when nothing new is at the offset', async () => {
  const a = rec('a')
  const { file, tail } = await tailOf(a)
  const pass = await tail.read(0)
  expect(pass.stat).toBeUndefined()
  const idle = await tail.read(pass.endPos)
  expect(idle.entries).toEqual([])
  expect(idle.stat?.size).toBe(Buffer.byteLength(a))
  await truncate(file, 0)
  const shrunk = await tail.read(pass.endPos)
  expect(shrunk.stat?.size).toBe(0)
})
