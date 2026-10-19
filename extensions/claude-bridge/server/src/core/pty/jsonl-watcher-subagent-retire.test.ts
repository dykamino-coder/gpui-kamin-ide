// Regression coverage for INC-2026-0069: retirement must preserve consumption.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import fs from 'node:fs'
import { appendFileSync, mkdirSync, mkdtempSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import fsp from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve, sep } from 'node:path'
import { setTimeout as settle } from 'node:timers/promises'
import { JsonlWatcher } from './jsonl-watcher'
import type { SubagentJsonlWatcher } from './subagent-jsonl-watcher'

describe('retired subagent transcripts', () => {
  let dir: string
  let watcher: JsonlWatcher
  let file: string
  let state: { filePath: string; slugDir: string; subagentWatchers: Map<string, SubagentJsonlWatcher> }
  const send = vi.fn<NonNullable<ConstructorParameters<typeof JsonlWatcher>[6]>>(() => true)
  const line = (uuid: string) => `${JSON.stringify({ type: 'assistant', uuid })}\n`

  beforeEach(() => {
    vi.useFakeTimers()
    vi.spyOn(fs, 'watch').mockImplementation(() => {
      throw new Error('use polling in tests')
    })
    send.mockClear()
    dir = mkdtempSync(join(tmpdir(), 'subagent-retire-'))
    mkdirSync(join(dir, 'conversation', 'subagents'), { recursive: true })
    file = join(dir, 'conversation', 'subagents', 'agent-one.jsonl')
    watcher = new JsonlWatcher(
      Date.now(),
      () => true,
      () => {},
      undefined,
      undefined,
      undefined,
      send,
    )
    state = watcher as unknown as typeof state
    state.filePath = join(dir, 'conversation.jsonl')
    state.slugDir = dir
  })

  afterEach(async () => {
    watcher.stop()
    await settle(30)
    vi.restoreAllMocks()
    vi.useRealTimers()
    // Delete only the exact temporary directory created by this test.
    expect(resolve(dir).startsWith(resolve(tmpdir()) + sep)).toBe(true)
    rmSync(dir, { recursive: true, force: true })
  })

  async function discover() {
    writeFileSync(file, line('old'))
    await watcher.scanSubagents()
    await waitForSends(1)
    expect(send).toHaveBeenCalledTimes(1)
  }

  async function waitForSends(count: number) {
    // Filesystem promises use real I/O even while watcher clocks are fake.
    // Wait for delivery rather than a fixed sleep, including on busy CI hosts.
    for (let i = 0; i < 300 && send.mock.calls.length < count; i++) await settle(10)
    await settle(0) // let the successful send commit its offset
    expect(send).toHaveBeenCalledTimes(count)
  }

  async function retire() {
    vi.setSystemTime(Date.now() + 5 * 60_000 + 1)
    await watcher.scanSubagents()
    await settle(50)
  }

  it('does not resend an unchanged retired subagent across scans', async () => {
    await discover()
    await retire()
    for (let i = 0; i < 5; i++) {
      await vi.advanceTimersByTimeAsync(4000)
      await watcher.scanSubagents()
      await settle(30)
    }
    expect(send).toHaveBeenCalledTimes(1)
    expect(state.subagentWatchers.size).toBe(0)
  })

  async function recheck() {
    vi.setSystemTime(Date.now() + 60_000)
    await watcher.scanSubagents()
    await waitForSends(2)
  }

  it('sends only appended lines after retirement', async () => {
    await discover()
    await retire()
    appendFileSync(file, line('new'))
    await recheck()
    expect(send).toHaveBeenCalledTimes(2)
    expect(send.mock.calls[1]?.[1]).toEqual([
      expect.objectContaining({ uuid: 'new', _pos: Buffer.byteLength(line('old')) }),
    ])
    expect(state.subagentWatchers.get(file)?.getLastOffset()).toBe(Buffer.byteLength(line('old') + line('new')))
  })

  it.each(['shrink', 'replace'] as const)('replays from zero after %s', async (kind) => {
    await discover()
    await retire()
    if (kind === 'replace') {
      // Keep the old inode alive; equal-sized replacement must still replay.
      renameSync(file, join(dir, 'previous.jsonl'))
      writeFileSync(file, line('NEW'))
    } else {
      writeFileSync(file, line('s'))
    }
    await recheck()
    expect(send).toHaveBeenCalledTimes(2)
    expect(send.mock.calls[1]?.[1]).toEqual([
      expect.objectContaining({ uuid: kind === 'replace' ? 'NEW' : 's', _pos: 0 }),
    ])
  })

  it('does not replay 50 finished subagents and bounds stat calls', async () => {
    for (let i = 0; i < 50; i++) {
      writeFileSync(join(dir, 'conversation', 'subagents', `agent-${i}.jsonl`), line(`old-${i}`))
    }
    await watcher.scanSubagents()
    await waitForSends(50)
    const stat = vi.spyOn(fsp, 'stat')
    await retire()
    expect(stat).toHaveBeenCalledTimes(50)
    expect(state.subagentWatchers.size).toBe(0)
    for (let cycle = 0; cycle < 3; cycle++) {
      for (let scan = 0; scan < 14; scan++) {
        stat.mockClear()
        await vi.advanceTimersByTimeAsync(4000)
        await watcher.scanSubagents()
        expect(stat).not.toHaveBeenCalled()
      }
      stat.mockClear()
      await vi.advanceTimersByTimeAsync(4000)
      await watcher.scanSubagents()
      expect(stat).toHaveBeenCalledTimes(50)
    }
    expect(send).toHaveBeenCalledTimes(50)
    watcher.stop()
    expect((watcher as unknown as { retiredSubagents: Map<string, unknown> }).retiredSubagents.size).toBe(0)
  })

  it('retries a refused resumed tail without advancing the offset', async () => {
    await discover()
    await retire()
    appendFileSync(file, line('new'))
    send.mockReturnValueOnce(false)
    await recheck()
    expect(state.subagentWatchers.get(file)?.getLastOffset()).toBe(Buffer.byteLength(line('old')))
    await vi.advanceTimersByTimeAsync(400)
    await waitForSends(3)
    expect(send.mock.calls[1]).toEqual(send.mock.calls[2])
    expect(state.subagentWatchers.get(file)?.getLastOffset()).toBe(Buffer.byteLength(line('old') + line('new')))
  })

  it('replays retired history on an explicit client reattach', async () => {
    await discover()
    await retire()
    vi.spyOn(watcher as unknown as { readInitialContent(): Promise<void> }, 'readInitialContent').mockResolvedValue()
    watcher.replayAll()
    await waitForSends(2)
    expect(send.mock.calls[1]?.[1]).toEqual([expect.objectContaining({ uuid: 'old', _pos: 0 })])
  })

  it('keeps first-discovery replay in 200-entry chunks', async () => {
    writeFileSync(file, Array.from({ length: 401 }, (_, i) => line(`entry-${i}`)).join(''))
    await watcher.scanSubagents()
    await waitForSends(1)
    expect(send.mock.calls[0]?.[1]).toHaveLength(200)
    await vi.advanceTimersByTimeAsync(10)
    await waitForSends(2)
    expect(send.mock.calls[1]?.[1]).toHaveLength(200)
    await vi.advanceTimersByTimeAsync(10)
    await waitForSends(3)
    expect(send.mock.calls[2]?.[1]).toHaveLength(1)
  })
})
