import { afterEach, expect, it } from 'vitest'
import { appendFile, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'
import type { JsonlEntry, JsonlStatus } from '../../shared/jsonl-types'
import { fingerprintTranscript } from './jsonl-fingerprint'

type Tagged = JsonlEntry & { _pos?: number; _posEnd?: number }
const watchers: JsonlWatcher[] = []
const dirs: string[] = []
afterEach(async () => {
  for (const w of watchers.splice(0)) w.stop()
  for (const d of dirs.splice(0)) await rm(d, { recursive: true, force: true })
})
const line = (uuid: string, type = 'assistant') => `${JSON.stringify({ type, uuid, message: { content: uuid } })}\n`

async function pagedWatcher(initial: string) {
  const dir = await mkdtemp(join(tmpdir(), 'live-cursor-'))
  dirs.push(dir)
  const file = join(dir, 'conv.jsonl')
  await writeFile(file, initial)
  const batches: Array<{ rows: Tagged[]; replay: boolean }> = []
  const statuses: JsonlStatus[] = []
  const w = new JsonlWatcher(
    Date.now(),
    (rows, replay) => {
      batches.push({ rows: rows as Tagged[], replay: replay === true })
      return true
    },
    (s) => {
      statuses.push(s)
    },
  )
  watchers.push(w)
  const state = w as unknown as { filePath: string; checkForNewContent(): Promise<void>; lastOffset: number }
  state.filePath = file
  return { file, w, state, batches, statuses }
}

it('tags live tail records with their byte span, so a reattach cursor resumes with no duplicate', async () => {
  const first = line('page-1')
  const f = await pagedWatcher(first)
  f.w.replayAll({ mode: 'paged' })
  await expect.poll(() => f.statuses.some((s) => s.replayComplete)).toBe(true)
  f.batches.length = 0

  const a = line('live-a')
  const b = line('live-b')
  await appendFile(f.file, a + b)
  await f.state.checkForNewContent()
  const live = f.batches.filter((x) => !x.replay).flatMap((x) => x.rows)
  expect(live.map((r) => [r.uuid, r._pos, r._posEnd])).toEqual([
    ['live-a', first.length, first.length + a.length],
    ['live-b', first.length + a.length, first.length + a.length + b.length],
  ])

  // The client mirror now ends at the last live record: a reattach with that
  // cursor must deliver nothing it already holds.
  const last = live.at(-1)!
  const head = (await fingerprintTranscript(f.file))!.head
  f.batches.length = 0
  f.statuses.length = 0
  f.w.replayAll({ mode: 'paged', cursor: { head, pos: last._posEnd!, lastPos: last._pos!, lastUuid: last.uuid } })
  await expect.poll(() => f.statuses.some((s) => s.replayComplete)).toBe(true)
  expect(f.batches.flatMap((x) => x.rows)).toEqual([])

  // And the tail continues exactly after it.
  await appendFile(f.file, line('live-c'))
  await f.state.checkForNewContent()
  expect(f.batches.flatMap((x) => x.rows).map((r) => r.uuid)).toEqual(['live-c'])
})

it('extends the paged segment index from the records the tail already parsed', async () => {
  const f = await pagedWatcher(line('page-1') + line('page-2'))
  f.w.replayAll({ mode: 'paged' })
  await expect.poll(() => f.statuses.some((s) => s.replayComplete)).toBe(true)
  const state = f.w as unknown as { indexOffset: number; lastOffset: number }
  await expect.poll(() => state.indexOffset).toBe(state.lastOffset)
  f.statuses.length = 0

  await appendFile(f.file, line('live-a') + line('live-b'))
  await f.state.checkForNewContent()
  expect(state.indexOffset).toBe(state.lastOffset)
  // Live counts are announced once per INDEX_ANNOUNCE_MS, not per record.
  expect(f.statuses.some((s) => s.segmentIndex)).toBe(false)
  await expect.poll(() => f.statuses.filter((s) => s.segmentIndex).at(-1)?.segmentIndex?.counts).toEqual([4])
  expect(f.statuses.filter((s) => s.segmentIndex)).toHaveLength(1)
})
