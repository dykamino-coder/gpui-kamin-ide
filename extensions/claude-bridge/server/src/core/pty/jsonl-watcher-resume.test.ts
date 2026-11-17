import { describe, it, expect, afterEach } from 'vitest'
import fsp from 'node:fs/promises'
import { mkdtempSync, writeFileSync, rmSync, utimesSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { JsonlWatcher, type TranscriptSwitchReason } from './jsonl-watcher'
import { CompactionScanner } from './compaction-scanner'
import type { JsonlEntry, JsonlStatus } from '../../shared/jsonl-types'

/** INC-2026-0102: an inline `/resume <id>` writes nothing to the resumed
 *  transcript, so discovery (newer file, born after the PTY) never selects it.
 *  The CLI reports the new identity only through SessionStart (source
 *  `resume`); the watcher must follow that report and nothing else. */
describe('JsonlWatcher — explicit resume handoff', () => {
  const dirs: string[] = []
  const watchers: JsonlWatcher[] = []
  afterEach(() => {
    for (const w of watchers.splice(0)) w.stop()
    for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
  })

  const RESUMED = '11111111-2222-4333-8444-555555555555'
  const CLEARED = '99999999-8888-4777-8666-555555555555'
  const rec = (o: unknown): string => `${JSON.stringify(o)}\n`
  const user = (uuid: string, text: string) =>
    rec({ type: 'user', uuid, timestamp: '2026-11-17T07:00:00.000Z', message: { role: 'user', content: text } })

  function setup() {
    const dir = mkdtempSync(join(tmpdir(), 'jsonl-resume-'))
    dirs.push(dir)
    const resumed = join(dir, `${RESUMED}.jsonl`)
    const cleared = join(dir, `${CLEARED}.jsonl`)
    writeFileSync(resumed, user('r1', 'history of the resumed chat'))
    // The resumed file is older and untouched by the resume itself.
    utimesSync(resumed, new Date(1000), new Date(1000))
    writeFileSync(cleared, user('c1', '/clear'))
    const entries: JsonlEntry[] = []
    const statuses: JsonlStatus[] = []
    const ids: Array<[string, TranscriptSwitchReason | undefined]> = []
    const w = new JsonlWatcher(
      Date.now(),
      (batch) => {
        entries.push(...batch)
        return true
      },
      (s) => {
        statuses.push(s)
      },
      undefined,
      undefined,
      (id, reason) => ids.push([id, reason]),
    )
    watchers.push(w)
    ;(w as unknown as { filePath: string }).filePath = cleared
    return { dir, resumed, cleared, entries, statuses, ids, w }
  }

  it('switches to the resumed transcript and replays its history', async () => {
    const { resumed, entries, statuses, ids, w } = setup()
    w.replayAll()
    await expect.poll(() => statuses.some((s) => s.replayComplete), { timeout: 5000 }).toBe(true)
    entries.length = 0
    statuses.length = 0

    expect(w.adoptResumedTranscript(RESUMED)).toBe(true)
    expect(w.getFilePath()).toBe(resumed)
    expect(ids).toEqual([[RESUMED, 'resume']])
    // The client is told to drop the cleared view and receives the old history.
    expect(statuses[0]).toMatchObject({ status: 'watching', filePath: resumed, compacted: true })
    await expect.poll(() => entries.some((e) => e.uuid === 'r1'), { timeout: 5000 }).toBe(true)
  })

  it('ignores ids that are not transcripts of this project', () => {
    const { cleared, ids, w } = setup()
    expect(w.adoptResumedTranscript('../escape')).toBe(false)
    expect(w.adoptResumedTranscript('aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee')).toBe(false)
    expect(w.adoptResumedTranscript(CLEARED)).toBe(false)
    expect(w.getFilePath()).toBe(cleared)
    expect(ids).toEqual([])
  })

  it('stays on the resumed file when the /clear file is discovered only after the resume', async () => {
    // Live order on 6.3.189: the resume arrived while the watcher still tailed
    // the resumed conversation, and the cleared file was found seconds later.
    const { resumed, cleared, statuses, ids, w } = setup()
    ;(w as unknown as { filePath: string }).filePath = resumed
    await new Promise((r) => setTimeout(r, 20))
    expect(w.adoptResumedTranscript(RESUMED)).toBe(false)
    const discovered = (w as unknown as { onDiscoveredNewer(p: string): void }).onDiscoveredNewer.bind(w)
    discovered(cleared)
    expect(w.getFilePath()).toBe(resumed)
    expect(ids).toEqual([])
    expect(statuses).toEqual([])
    // A transcript created after the resume (a later /clear or compaction) is followed.
    await new Promise((r) => setTimeout(r, 20))
    const next = join(dirname(resumed), 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee.jsonl')
    writeFileSync(next, user('n1', '/clear'))
    discovered(next)
    expect(w.getFilePath()).toBe(next)
  })

  it('keeps discovery from handing the view back to the newer abandoned file', async () => {
    const { dir, resumed, cleared } = setup()
    const scanner = new CompactionScanner()
    const newer: string[] = []
    const left = new Set<string>([cleared])
    const unsub = scanner.subscribe({
      dir,
      currentFilePath: resumed,
      sessionCreatedAt: 0,
      left,
      onNewer: (p) => newer.push(p),
    })
    try {
      await scanner.sweep()
      await scanner.sweep()
      expect(newer).toEqual([])
      // A later /clear creates a fresh file: discovery still follows it.
      const next = join(dir, 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee.jsonl')
      await fsp.writeFile(next, user('n1', '/clear'))
      await expect
        .poll(async () => {
          await scanner.sweep()
          return newer.includes(next)
        })
        .toBe(true)
      expect(newer).not.toContain(cleared)
    } finally {
      unsub()
    }
  })
})
