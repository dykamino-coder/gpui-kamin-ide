import { afterEach, expect, it, vi } from 'vitest'
import fs from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve, sep } from 'node:path'
import { setTimeout as settle } from 'node:timers/promises'
import * as lines from './jsonl-lines'
import { SubagentJsonlWatcher } from './subagent-jsonl-watcher'
import type { JsonlEntry } from '../../shared/jsonl-types'

let watcher: SubagentJsonlWatcher | undefined
let dir: string | undefined
afterEach(async () => {
  watcher?.stop()
  await settle(20)
  vi.restoreAllMocks()
  vi.useRealTimers()
  if (dir) {
    expect(resolve(dir).startsWith(resolve(tmpdir()) + sep)).toBe(true)
    fs.rmSync(dir, { recursive: true, force: true })
  }
})

it('counts the batch gap from delivery even when EOF completion finishes later', async () => {
  vi.useFakeTimers()
  vi.spyOn(fs, 'watch').mockImplementation(() => {
    throw new Error('poll only')
  })
  dir = fs.mkdtempSync(join(tmpdir(), 'subagent-batch-deadline-'))
  const file = join(dir, 'agent.jsonl')
  fs.writeFileSync(
    file,
    Array.from({ length: 401 }, (_, i) => JSON.stringify({ type: 'assistant', uuid: `${i}` }) + '\n').join(''),
  )
  let finishEof!: () => void
  let reachedEof = false
  const eof = new Promise<void>((resolve) => {
    finishEof = resolve
  })
  const original = lines.readJsonlLines
  vi.spyOn(lines, 'readJsonlLines').mockImplementation(async function* (...args) {
    yield* original(...args)
    reachedEof = true
    // File close / cooperative reader completion may finish after the gap.
    await eof
  })
  const send = vi.fn<(entries: JsonlEntry[]) => boolean>(() => true)
  watcher = new SubagentJsonlWatcher('agent', file, send)
  await watcher.start()
  async function waitUntil(predicate: () => boolean) {
    for (let i = 0; i < 300 && !predicate(); i++) await settle(10)
    expect(predicate()).toBe(true)
  }
  await waitUntil(() => send.mock.calls.length === 1)
  expect(send.mock.calls[0]?.[0]).toHaveLength(200)
  await vi.advanceTimersByTimeAsync(10)
  await waitUntil(() => reachedEof)
  expect(send).toHaveBeenCalledTimes(2)
  expect(send.mock.calls[1]?.[0]).toHaveLength(200)
  await vi.advanceTimersByTimeAsync(10)
  finishEof()
  await waitUntil(() => send.mock.calls.length === 3)
  expect(send.mock.calls[2]?.[0]).toHaveLength(1)
  expect(watcher.getLastOffset()).toBe(fs.statSync(file).size)
})
