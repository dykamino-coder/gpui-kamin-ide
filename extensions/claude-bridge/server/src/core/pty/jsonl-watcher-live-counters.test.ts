// The dashboard's live session counters (User/Asst/Context/Total) as the real
// watcher feeds them: a long paged session with a live tail, reattaches,
// a resume onto a transcript with history, and a client that is not attached.
import { afterEach, expect, it } from 'vitest'
import { appendFile, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'
import type { JsonlEntry } from '../../shared/jsonl-types'
import { accumulateJsonlStats, resetJsonlStats } from './session-jsonl-stats'
import type { PtySession } from './types'

const watchers: JsonlWatcher[] = []
const dirs: string[] = []
afterEach(async () => {
  for (const w of watchers.splice(0)) w.stop()
  for (const d of dirs.splice(0)) await rm(d, { recursive: true, force: true })
})

const user = (i: number) =>
  JSON.stringify({ type: 'user', uuid: `u${i}`, message: { role: 'user', content: `q${i}` } }) + '\n'
const reply = (i: number, context: number) =>
  JSON.stringify({
    type: 'assistant',
    uuid: `a${i}`,
    message: {
      id: `msg-${i}`,
      role: 'assistant',
      content: [{ type: 'text', text: 'ok' }],
      usage: {
        input_tokens: 10,
        cache_read_input_tokens: context - 10,
        cache_creation_input_tokens: 0,
        output_tokens: 5,
      },
    },
  }) + '\n'
const turns = (from: number, to: number) => {
  let s = ''
  for (let i = from; i < to; i++) s += user(i) + reply(i, 1000 + i)
  return s
}

async function setup(initial: string, opts: { writable?: () => boolean } = {}) {
  const dir = await mkdtemp(join(tmpdir(), 'live-counters-'))
  dirs.push(dir)
  const file = join(dir, 'conv.jsonl')
  await writeFile(file, initial)
  const session = {} as PtySession
  resetJsonlStats(session)
  const w = new JsonlWatcher(
    Date.now(),
    () => true,
    () => {},
    undefined,
    undefined,
    undefined,
    undefined,
    () => resetJsonlStats(session),
    (e: JsonlEntry) => accumulateJsonlStats(session, e),
    opts.writable,
    { mode: 'paged' },
  )
  watchers.push(w)
  Object.assign(w, { filePath: file })
  return { w, file, session }
}

const tail = (w: JsonlWatcher) => (w as unknown as { checkForNewContent(): Promise<void> }).checkForNewContent()

it('counts a long paged session once across its tail and repeated reattaches', async () => {
  // Larger than one history page: the counters must cover the whole file.
  const { w, file, session } = await setup(turns(0, 3000))
  w.replayAll({ mode: 'paged' })
  await expect.poll(() => session.userMessages, { timeout: 15_000 }).toBe(3000)
  expect(session.assistantMessages).toBe(3000)
  expect(session.contextTokens).toBe(1000 + 2999)

  await appendFile(file, turns(3000, 3010))
  await tail(w)
  await expect.poll(() => session.userMessages).toBe(3010)

  // Reattach (client reconnect) twice, with new lines in between.
  w.replayAll({ mode: 'paged' })
  await appendFile(file, turns(3010, 3012))
  await tail(w)
  w.replayAll({ mode: 'paged' })
  await expect.poll(() => session.userMessages, { timeout: 15_000 }).toBe(3012)
  await new Promise((r) => setTimeout(r, 300))
  expect(session.userMessages).toBe(3012)
  expect(session.assistantMessages).toBe(3012)
  expect(session.contextTokens).toBe(1000 + 3011)
  expect(session.totalTokens).toBe(Array.from({ length: 3012 }, (_, i) => 1000 + i + 5).reduce((a, b) => a + b, 0))
})

it('counts a resumed transcript with its history before the first new turn', async () => {
  const { w, session } = await setup(turns(0, 40))
  w.replayAll({ mode: 'paged' })
  await expect.poll(() => session.userMessages).toBe(40)
  expect(session.contextTokens).toBe(1039)
})

it('fills the counters while no client is attached', async () => {
  // After a server restart a session can run with its client gone for hours.
  const { w, file, session } = await setup(turns(0, 25), { writable: () => false })
  w.replayAll({ mode: 'paged' })
  await expect.poll(() => session.userMessages, { timeout: 5_000 }).toBe(25)
  await appendFile(file, turns(25, 27))
  // The client is still blocked; server counters must continue independently.
  void tail(w)
  await expect.poll(() => session.userMessages, { timeout: 5_000 }).toBe(27)
  expect(session.assistantMessages).toBe(27)
})
