import { afterEach, describe, expect, it } from 'vitest'
import { appendFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { SubagentJsonlWatcher } from './subagent-jsonl-watcher'
import type { SendOutcome } from './jsonl-replay'

// INC-2026-0078 F09: a refused first frame used to return before the watch,
// poll or retry were armed — the subagent never updated again until a new
// resume. Delivery now resumes on its own and never repeats confirmed batches.

const dirs: string[] = []
const watchers: SubagentJsonlWatcher[] = []
afterEach(() => {
  for (const w of watchers.splice(0)) w.stop()
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
})

const line = (uuid: string, text = 'x') =>
  JSON.stringify({ type: 'assistant', uuid, message: { content: text } }) + '\n'

function setup(content: string, send: (n: number, entries: { uuid?: string }[]) => SendOutcome | boolean) {
  const dir = mkdtempSync(join(tmpdir(), 'subagent-retry-'))
  dirs.push(dir)
  const file = join(dir, 'agent-a.jsonl')
  writeFileSync(file, content)
  const got: string[][] = []
  let calls = 0
  const w = new SubagentJsonlWatcher('a', file, (entries) => {
    const r = send(calls++, entries as { uuid?: string }[])
    if (r === true || r === 'sent') got.push(entries.map((e) => String(e.uuid)))
    return r
  })
  watchers.push(w)
  return { w, file, got }
}

describe('subagent watcher delivery', () => {
  it('recovers from a refused first frame without a new resume', async () => {
    const s = setup(line('a1') + line('a2'), (n) => (n === 0 ? 'backpressured' : 'sent'))
    await s.w.start()
    await expect.poll(() => s.got.flat(), { timeout: 3000 }).toEqual(['a1', 'a2'])
    appendFileSync(s.file, line('a3'))
    await expect.poll(() => s.got.flat(), { timeout: 3000 }).toEqual(['a1', 'a2', 'a3'])
  })

  it('keeps confirmed batches when a later batch is refused, and never resends them', async () => {
    const rows = Array.from({ length: 450 }, (_, i) => line(`r${i}`)).join('')
    const s = setup(rows, (n) => (n === 1 ? false : 'sent'))
    await s.w.start()
    await expect.poll(() => s.got.flat().length, { timeout: 5000 }).toBe(450)
    expect(new Set(s.got.flat()).size).toBe(450) // no duplicates
    expect(s.got[0]).toHaveLength(200)
  })

  it('replaces a record that can never fit with a stub instead of retrying forever', async () => {
    const s = setup(line('small') + line('huge', 'H'.repeat(4096)) + line('after'), (_n, entries) =>
      JSON.stringify(entries).length > 2048 ? 'oversized' : 'sent',
    )
    await s.w.start()
    await expect.poll(() => s.got.flat(), { timeout: 3000 }).toEqual(['small', 'huge', 'after'])
  })
})
