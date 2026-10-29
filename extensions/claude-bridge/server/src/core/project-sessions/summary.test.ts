import fsp from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, expect, it, vi } from 'vitest'
import { COUNT_SLICE_BYTES, summarize } from './summary'

const dirs: string[] = []
afterEach(async () => {
  vi.restoreAllMocks()
  for (const d of dirs.splice(0)) await fsp.rm(d, { recursive: true, force: true })
})

it('counts bounded slices once under concurrent requests, then reads only the appended delta', async () => {
  const dir = await fsp.mkdtemp(join(tmpdir(), 'summary-cursor-'))
  dirs.push(dir)
  const file = join(dir, 'conv.jsonl')
  const line =
    JSON.stringify({ type: 'user', timestamp: '2026-01-01T00:00:00Z', message: { content: 'x'.repeat(1000) } }) + '\n'
  await fsp.writeFile(file, line.repeat(1200))
  const st = await fsp.stat(file)
  expect((await summarize(file, st)).lines).toBeNull()
  // These requests share the last immutable cursor, not mutable counters.
  await Promise.all(Array.from({ length: 10 }, () => summarize(file, st)))
  let result = await summarize(file, st)
  for (let i = 0; result.lines === null && i < Math.ceil(st.size / COUNT_SLICE_BYTES); i++)
    result = await summarize(file, st)
  expect(result.lines).toBe(1200)
  const open = vi.spyOn(fsp, 'open')
  expect((await summarize(file, st)).lines).toBe(1200)
  expect(open).not.toHaveBeenCalled()
  await fsp.appendFile(file, line)
  expect((await summarize(file, await fsp.stat(file))).lines).toBe(1201)
})

it('invalidates summaries on same-size rewrites and file replacement', async () => {
  const dir = await fsp.mkdtemp(join(tmpdir(), 'summary-rewrite-'))
  dirs.push(dir)
  const file = join(dir, 'conv.jsonl')
  const row = (text: string) => JSON.stringify({ type: 'user', message: { content: text } }) + '\n'
  await fsp.writeFile(file, row('before'))
  const st = await fsp.stat(file)
  expect((await summarize(file, st)).title).toBe('before')
  await fsp.writeFile(file, row('after!'))
  await fsp.utimes(file, new Date(), new Date(st.mtimeMs + 2000))
  expect((await summarize(file, await fsp.stat(file))).title).toBe('after!')
  await fsp.rename(file, join(dir, 'old.jsonl'))
  await fsp.writeFile(file, row('newest'))
  expect((await summarize(file, await fsp.stat(file))).title).toBe('newest')
})
