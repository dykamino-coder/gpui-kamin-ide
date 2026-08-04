import { describe, it, expect, afterEach } from 'vitest'
import { mkdtempSync, writeFileSync, appendFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'
import type { JsonlStatus } from '../../shared/jsonl-types'

/** A live `/compact` appends a compact_boundary to the SAME transcript file, so
 *  it rides the tail reader — not switchToFile (a new file, which re-runs the
 *  full replay). The segment strip is driven by the server's authoritative
 *  segmentIndex, which used to be built ONCE at replayComplete: without a live
 *  rebuild the strip kept showing the pre-compact "Current" segment (stale count,
 *  no new segment) until a reload. */
describe('JsonlWatcher — live /compact rebuilds the segment index', () => {
  const dirs: string[] = []
  const watchers: JsonlWatcher[] = []
  afterEach(() => {
    for (const w of watchers.splice(0)) w.stop()
    for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
  })

  const rec = (o: unknown): string => `${JSON.stringify(o)}\n`
  const user = (uuid: string, ts: string, text: string) =>
    rec({ type: 'user', uuid, timestamp: ts, message: { role: 'user', content: text } })
  const assistant = (uuid: string, ts: string, text: string) =>
    rec({ type: 'assistant', uuid, timestamp: ts, message: { role: 'assistant', content: [{ type: 'text', text }] } })
  const boundary = (uuid: string, ts: string) =>
    rec({ type: 'system', subtype: 'compact_boundary', uuid, timestamp: ts })

  const lastIndex = (statuses: JsonlStatus[]): JsonlStatus['segmentIndex'] =>
    statuses.filter((s) => s.segmentIndex).pop()?.segmentIndex

  it('resends a segmentIndex with the new boundary when one is appended live', async () => {
    const statuses: JsonlStatus[] = []
    const dir = mkdtempSync(join(tmpdir(), 'jsonl-live-compact-'))
    dirs.push(dir)
    const file = join(dir, 'conv.jsonl')
    // One prior compaction already in the file → initial index has 1 boundary.
    writeFileSync(
      file,
      user('u1', '2026-07-24T00:00:00.000Z', 'hello') +
        boundary('b1', '2026-07-24T00:01:00.000Z') +
        assistant('a1', '2026-07-24T00:02:00.000Z', 'hi'),
    )

    const w = new JsonlWatcher(Date.now(), () => true, (s) => { statuses.push(s) })
    watchers.push(w)
    ;(w as unknown as { filePath: string }).filePath = file

    w.replayAll()
    await new Promise((r) => setTimeout(r, 150))

    // Initial replay: index carries exactly the one existing boundary.
    expect(lastIndex(statuses)?.boundaries).toHaveLength(1)

    // A live /compact: append a SECOND boundary + its summary turn, then let the
    // tail read it (drive checkForNewContent directly — no poll-timing flake).
    appendFileSync(
      file,
      boundary('b2', '2026-07-24T01:00:00.000Z') +
        assistant('a2', '2026-07-24T01:01:00.000Z', 'after compact'),
    )
    statuses.length = 0
    await (w as unknown as { checkForNewContent(): Promise<void> }).checkForNewContent()
    await new Promise((r) => setTimeout(r, 150)) // rebuild is fire-and-forget

    // The strip's source now reflects BOTH boundaries — this is the fix.
    const after = lastIndex(statuses)
    expect(after?.boundaries).toHaveLength(2)
    // counts.length === boundaries.length + 1 (pre + one per segment).
    expect(after?.counts).toHaveLength(3)
  })

  it('does not rebuild the index for an ordinary (non-boundary) tail append', async () => {
    const statuses: JsonlStatus[] = []
    const dir = mkdtempSync(join(tmpdir(), 'jsonl-live-nocompact-'))
    dirs.push(dir)
    const file = join(dir, 'conv.jsonl')
    writeFileSync(file, user('u1', '2026-07-24T00:00:00.000Z', 'hello'))

    const w = new JsonlWatcher(Date.now(), () => true, (s) => { statuses.push(s) })
    watchers.push(w)
    ;(w as unknown as { filePath: string }).filePath = file

    w.replayAll()
    await new Promise((r) => setTimeout(r, 150))

    // A plain assistant turn — no boundary, so no segmentIndex resend.
    appendFileSync(file, assistant('a1', '2026-07-24T00:01:00.000Z', 'hi'))
    statuses.length = 0
    await (w as unknown as { checkForNewContent(): Promise<void> }).checkForNewContent()
    await new Promise((r) => setTimeout(r, 100))

    expect(statuses.some((s) => s.segmentIndex)).toBe(false)
  })
})
