import { afterEach, describe, expect, it, vi } from 'vitest'
import { appendFileSync, mkdtempSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'
import type { JsonlStatus } from '../../shared/jsonl-types'

const rec = (value: unknown): string => `${JSON.stringify(value)}\n`
const user = (uuid: string, text = 'synthetic'): string =>
  rec({ type: 'user', uuid, timestamp: '2026-01-01T00:00:00Z', message: { role: 'user', content: text } })
const boundary = (uuid: string): string =>
  rec({
    type: 'system',
    subtype: 'compact_boundary',
    uuid,
    timestamp: uuid === 'b1' ? '2026-01-01T01:00:00Z' : '2026-01-01T02:00:00Z',
  })
const dirs: string[] = []
const watchers: JsonlWatcher[] = []
afterEach(() => {
  for (const w of watchers.splice(0)) w.stop()
  vi.restoreAllMocks()
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
})

async function fixture() {
  const dir = mkdtempSync(join(tmpdir(), 'jsonl-rewrite-'))
  dirs.push(dir)
  const file = join(dir, 'conversation.jsonl')
  const prefix = user('first') + boundary('b1')
  writeFileSync(file, prefix + user('old-tail', 'x'.repeat(50000)))
  const statuses: JsonlStatus[] = []
  const delivered: any[] = []
  let accepting = true
  let stats: string[] = []
  let resets = 0
  const identity = vi.fn()
  const w = new JsonlWatcher(
    Date.now(),
    (entries) => {
      if (!accepting) return false
      delivered.push(...entries)
      return true
    },
    (s) => {
      statuses.push(s)
      if (s.compacted) delivered.length = 0
    },
    undefined,
    undefined,
    identity,
    undefined,
    () => {
      resets++
      stats = []
    },
    (e) => {
      stats.push(e.uuid ?? '')
    },
  )
  watchers.push(w)
  const state = w as any
  state.filePath = file
  const suspend = (): void => {
    if (state.tailTimer) {
      clearInterval(state.tailTimer)
      state.tailTimer = null
    }
    if (state.tailWatcher) {
      state.tailWatcher.close()
      state.tailWatcher = null
    }
  }
  const complete = async (): Promise<void> => {
    await expect.poll(() => statuses.some((s) => s.replayComplete), { timeout: 3000 }).toBe(true)
    suspend()
  }
  w.replayAll()
  await complete()
  statuses.length = 0
  delivered.length = 0
  return {
    file,
    dir,
    prefix,
    w,
    state,
    statuses,
    delivered,
    identity,
    complete,
    accepting: (v: boolean) => {
      accepting = v
    },
    stats: () => stats,
    resets: () => resets,
  }
}

describe('INC-0068 authoritative replay after a same-path rewrite', () => {
  it('recovers a compact boundary below the old cursor and keeps later messages live', async () => {
    const f = await fixture()
    writeFileSync(f.file, f.prefix + boundary('b2') + user('summary'))
    await f.state.checkForNewContent()
    await f.complete()
    expect(f.statuses.find((s) => s.compacted)).toMatchObject({ status: 'watching', replayComplete: false })
    expect(f.statuses.filter((s) => s.segmentIndex).at(-1)?.segmentIndex?.boundaries).toHaveLength(2)
    expect(f.delivered.some((e) => e.uuid === 'b2')).toBe(true)
    expect(f.identity).not.toHaveBeenCalled()
    expect(f.resets()).toBe(2)
    expect(f.stats()).toEqual(['first', 'b1', 'b2', 'summary'])
    appendFileSync(f.file, user('later'))
    await f.state.checkForNewContent()
    expect(f.delivered.some((e) => e.uuid === 'later')).toBe(true)
    expect(f.stats()).toEqual(['first', 'b1', 'b2', 'summary', 'later'])
  })
  it('resets an empty rewrite, completes it and follows later growth', async () => {
    const f = await fixture()
    writeFileSync(f.file, '')
    await f.state.checkForNewContent()
    await f.complete()
    expect(f.statuses.some((s) => s.compacted)).toBe(true)
    expect(f.state.lastOffset).toBe(0)
    appendFileSync(f.file, user('new'))
    await f.state.checkForNewContent()
    expect(f.delivered.map((e) => e.uuid)).toEqual(['new'])
  })
  it('detects an atomically replaced file even when it grew beyond the old cursor', async () => {
    const f = await fixture()
    const replacement = join(f.dir, 'replacement.jsonl')
    writeFileSync(replacement, f.prefix + boundary('b2') + user('replacement', 'y'.repeat(60000)))
    renameSync(replacement, f.file)
    await f.state.checkForNewContent()
    await f.complete()
    expect(f.statuses.some((s) => s.compacted)).toBe(true)
    expect(f.statuses.filter((s) => s.segmentIndex).at(-1)?.segmentIndex?.boundaries).toHaveLength(2)
  })
  it('honors backpressure during recovery instead of losing rewritten history', async () => {
    const f = await fixture()
    f.accepting(false)
    writeFileSync(f.file, f.prefix + boundary('b2') + user('summary'))
    await f.state.checkForNewContent()
    expect(f.statuses.some((s) => s.replayComplete)).toBe(false)
    f.accepting(true)
    await f.complete()
    expect(f.delivered.some((e) => e.uuid === 'summary')).toBe(true)
    expect(f.stats()).toEqual(['first', 'b1', 'b2', 'summary'])
  })
  it('rejects a tail result when the file was replaced during its asynchronous read', async () => {
    const f = await fixture()
    const read = f.state.readEntriesFromOffset.bind(f.state)
    let release!: () => void
    const gate = new Promise<void>((r) => {
      release = r
    })
    let entered!: () => void
    const started = new Promise<void>((r) => {
      entered = r
    })
    vi.spyOn(f.state, 'readEntriesFromOffset').mockImplementationOnce(async (...args: unknown[]) => {
      const result = await read(...args)
      entered()
      await gate
      return result
    })
    appendFileSync(f.file, user('obsolete-tail'))
    const pending = f.state.checkForNewContent()
    await started
    const replacement = join(f.dir, 'new.jsonl')
    writeFileSync(replacement, user('fresh', 'z'.repeat(60000)))
    renameSync(replacement, f.file)
    release()
    await pending
    await f.complete()
    expect(f.delivered.some((e) => e.uuid === 'obsolete-tail')).toBe(false)
    expect(f.delivered.some((e) => e.uuid === 'fresh')).toBe(true)
    expect(f.stats()).toEqual(['fresh'])
  })
  it('drops an asynchronous tail read superseded by a replay on the same path', async () => {
    const f = await fixture()
    let release!: () => void
    const gate = new Promise<void>((r) => {
      release = r
    })
    let entered!: () => void
    const started = new Promise<void>((r) => {
      entered = r
    })
    const oldOffset = f.state.lastOffset
    vi.spyOn(f.state, 'readEntriesFromOffset').mockImplementationOnce(async () => {
      entered()
      await gate
      return {
        entries: [{ type: 'user', uuid: 'stale-tail', message: { content: 'obsolete' } }],
        endPos: oldOffset + 1000,
      }
    })
    appendFileSync(f.file, user('old-pending'))
    const pending = f.state.checkForNewContent()
    await started
    writeFileSync(f.file, user('fresh'))
    f.w.replayAll()
    await f.complete()
    const freshOffset = f.state.lastOffset
    release()
    await pending
    expect(f.delivered.some((e) => e.uuid === 'stale-tail')).toBe(false)
    expect(f.state.lastOffset).toBe(freshOffset)
    expect(f.stats()).toEqual(['fresh'])
  })
})
