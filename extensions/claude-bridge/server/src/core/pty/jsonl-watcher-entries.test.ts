import { afterEach, expect, it } from 'vitest'
import { appendFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'

// Read receipts look for the stored prompt row in what the watcher reads. A
// new chat's first prompt is in the FIRST read of its file, not in the tail.
const dirs: string[] = []
const watchers: JsonlWatcher[] = []
afterEach(() => {
  for (const w of watchers.splice(0)) w.stop()
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
})

const row = (uuid: string) =>
  JSON.stringify({ type: 'user', uuid, timestamp: new Date().toISOString(), message: { content: uuid } }) + '\n'

it('offers the first read of a file and later appends to onEntries, even with no writable client', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'jsonl-entries-'))
  dirs.push(dir)
  const file = join(dir, 'chat.jsonl')
  writeFileSync(file, row('first-prompt'))
  let writable = false
  const w = new JsonlWatcher(
    0,
    () => writable,
    () => {},
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    () => writable,
  )
  watchers.push(w)
  const seen: string[] = []
  w.onEntries = (entries) => seen.push(...entries.map((e) => (e as { uuid?: string }).uuid ?? ''))
  ;(w as unknown as { filePath: string }).filePath = file
  w.replayAll()
  await new Promise((r) => setTimeout(r, 150))
  expect(seen).toContain('first-prompt')
  writable = true
  await new Promise((r) => setTimeout(r, 300))
  appendFileSync(file, row('second-prompt'))
  for (let i = 0; i < 40 && !seen.includes('second-prompt'); i++) await new Promise((r) => setTimeout(r, 100))
  expect(seen).toContain('second-prompt')
})
