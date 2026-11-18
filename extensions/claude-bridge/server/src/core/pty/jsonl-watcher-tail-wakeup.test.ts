// Live tail wake-ups: a change signal that lands while a tail read is running
// must not be lost (it left the newest records waiting for the 10s safety
// poll under load), and an outside poke never reads before tailing started.
import { afterEach, expect, it } from 'vitest'
import { appendFileSync } from 'node:fs'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'
import type { JsonlEntry } from '../../shared/jsonl-types'

const watchers: JsonlWatcher[] = []
const dirs: string[] = []
afterEach(async () => {
  for (const w of watchers.splice(0)) w.stop()
  for (const d of dirs.splice(0)) await rm(d, { recursive: true, force: true })
})

const line = (i: number) =>
  JSON.stringify({
    type: 'assistant',
    uuid: `a${i}`,
    message: { role: 'assistant', content: [{ type: 'text', text: `t${i}` }] },
  }) + '\n'

async function setup(onSend: (entries: JsonlEntry[]) => void) {
  const dir = await mkdtemp(join(tmpdir(), 'tail-wakeup-'))
  dirs.push(dir)
  const file = join(dir, 'conv.jsonl')
  await writeFile(file, '')
  const w = new JsonlWatcher(
    Date.now(),
    (entries) => {
      onSend(entries)
      return true
    },
    () => {},
  )
  watchers.push(w)
  Object.assign(w, { filePath: file })
  return { w, file }
}
const tail = (w: JsonlWatcher) => (w as unknown as { checkForNewContent(): Promise<void> }).checkForNewContent()

it('re-reads when a write lands while a tail read is running', async () => {
  const seen: string[] = []
  let file = ''
  let w!: JsonlWatcher
  ;({ w, file } = await setup((entries) => {
    seen.push(...entries.map((e) => e.uuid!))
    // The CLI appends again and its fs event fires while this read still runs.
    if (seen.length === 1) {
      appendFileSync(file, line(2) + line(3))
      void tail(w)
    }
  }))
  appendFileSync(file, line(1))
  await tail(w)
  // No safety poll or fs.watch runs in this test: only the deferred re-read
  // can deliver the records written during the first read.
  await expect.poll(() => seen, { timeout: 2000 }).toEqual(['a1', 'a2', 'a3'])
})

it('ignores a poke until live tailing has started', async () => {
  const seen: string[] = []
  const { w, file } = await setup((entries) => seen.push(...entries.map((e) => e.uuid!)))
  appendFileSync(file, line(1))
  // Before tailing the initial replay owns the file; a hook must not race it.
  w.poke()
  await new Promise((r) => setTimeout(r, 100))
  expect(seen).toEqual([])
})
