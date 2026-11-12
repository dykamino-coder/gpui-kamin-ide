import { afterEach, describe, expect, it } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import type { JsonlEntry } from '../../shared/jsonl-types'
import {
  activeSegment,
  loadManifest,
  readManifest,
  resetManifestCacheForTests,
  saveManifest,
  type ConversationManifest,
} from './manifest'
import { SegmentPump } from './segment-pump'
import { ENGINE_SWITCH_SUBTYPE, UnifiedLog, lastUuidIn, seedUnifiedLog } from './unified-log'

const roots: string[] = []
afterEach(() => {
  resetManifestCacheForTests()
  for (const r of roots.splice(0)) fs.rmSync(r, { recursive: true, force: true })
})
function tmp(): string {
  const r = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-conv-'))
  roots.push(r)
  return r
}
const entry = (uuid: string, parentUuid: string | null, extra: object = {}) =>
  ({ type: 'user', uuid, parentUuid, message: { role: 'user', content: uuid }, ...extra }) as unknown as JsonlEntry
const lines = (file: string) =>
  fs
    .readFileSync(file, 'utf8')
    .trim()
    .split('\n')
    .map((l) => JSON.parse(l) as Record<string, unknown>)

describe('manifest', () => {
  const manifest = (dir: string): ConversationManifest => ({
    version: 1,
    id: 'L',
    dir,
    createdAt: new Date().toISOString(),
    segments: [
      {
        engine: 'claude',
        nativeId: 'A',
        settingsDir: dir,
        model: null,
        startedAt: '',
        pump: { file: null, pos: 0 },
      },
    ],
    active: 0,
  })

  it('round-trips and shares one in-memory copy per conversation', () => {
    const dir = tmp()
    saveManifest(manifest(dir))
    const a = loadManifest(dir)!
    expect(activeSegment(a).nativeId).toBe('A')
    expect(loadManifest(path.join(dir, '.'))).toBe(a)
    resetManifestCacheForTests()
    expect(readManifest(dir)).toMatchObject({ id: 'L', active: 0 })
  })

  it('rejects a manifest whose active segment does not exist', () => {
    const dir = tmp()
    fs.writeFileSync(path.join(dir, 'conversation.json'), JSON.stringify({ ...manifest(dir), active: 3 }))
    expect(readManifest(dir)).toBeNull()
    expect(loadManifest(tmp())).toBeNull()
  })
})

describe('unified log', () => {
  it('seeds from a transcript without its torn last line', () => {
    const dir = tmp()
    const src = path.join(dir, 'a.jsonl')
    fs.writeFileSync(src, JSON.stringify(entry('a1', null)) + '\n{"uuid":"tor')
    const log = path.join(dir, 'L.jsonl')
    seedUnifiedLog(log, src)
    expect(lines(log).map((e) => e.uuid)).toEqual(['a1'])
    seedUnifiedLog(path.join(dir, 'empty.jsonl'), path.join(dir, 'missing.jsonl'))
    expect(fs.readFileSync(path.join(dir, 'empty.jsonl'), 'utf8')).toBe('')
  })

  it('chains a new segment onto the log and keeps its inner parents', () => {
    const dir = tmp()
    const file = path.join(dir, 'L.jsonl')
    fs.writeFileSync(file, JSON.stringify(entry('a1', null)) + '\n')
    const log = new UnifiedLog(file)
    const marker = log.appendSwitchMarker(
      { engine: 'codex', model: 'gpt-6.1-sol', label: 'gpt-6.1-sol' },
      'L',
    ) as unknown as {
      uuid: string
    }
    log.append([
      entry('b1', null),
      entry('b2', 'b1'),
      entry('side', 'elsewhere', { isSidechain: true }),
      entry('b3', 'not-in-log'),
    ])
    const out = lines(file)
    expect(out[1]).toMatchObject({ subtype: ENGINE_SWITCH_SUBTYPE, parentUuid: 'a1', content: '→ gpt-6.1-sol' })
    expect(out[2]).toMatchObject({ uuid: 'b1', parentUuid: marker.uuid })
    expect(out[3]).toMatchObject({ uuid: 'b2', parentUuid: 'b1' })
    expect(out[4]).toMatchObject({ uuid: 'side', parentUuid: 'elsewhere' })
    expect(out[5]).toMatchObject({ uuid: 'b3', parentUuid: 'b2' })
    expect(lastUuidIn(file)).toBe('b3')
  })

  it('does not append an entry it already holds (a pump position saved late before a crash)', () => {
    const dir = tmp()
    const file = path.join(dir, 'L.jsonl')
    new UnifiedLog(file).append([entry('a1', null), entry('a2', 'a1')])
    // A restarted server copies the same lines again.
    const again = new UnifiedLog(file)
    again.append([entry('a1', null), entry('a2', 'a1'), entry('a3', 'a2')])
    expect(lines(file).map((e) => e.uuid)).toEqual(['a1', 'a2', 'a3'])
    expect(lines(file)[2]).toMatchObject({ parentUuid: 'a2' })
  })

  it('cuts a torn last line before appending instead of gluing onto it', () => {
    const dir = tmp()
    const file = path.join(dir, 'L.jsonl')
    fs.writeFileSync(file, JSON.stringify(entry('a1', null)) + '\n' + '{"uuid":"a2","parentUu')
    const log = new UnifiedLog(file)
    log.append([entry('a2', 'a1'), entry('a3', 'a2')])
    expect(lines(file).map((e) => e.uuid)).toEqual(['a1', 'a2', 'a3'])
  })

  it('cuts a torn line without any newline before it', () => {
    const dir = tmp()
    const file = path.join(dir, 'L.jsonl')
    fs.writeFileSync(file, '{"uuid":"a1","par')
    new UnifiedLog(file).append([entry('a1', null)])
    expect(lines(file).map((e) => e.uuid)).toEqual(['a1'])
  })

  it('takes the last main-chain entry, not a sub-agent side chain', () => {
    const dir = tmp()
    const file = path.join(dir, 'L.jsonl')
    fs.writeFileSync(
      file,
      [entry('a1', null), entry('side', 'x', { isSidechain: true })].map((e) => JSON.stringify(e)).join('\n') + '\n',
    )
    expect(lastUuidIn(file)).toBe('a1')
    new UnifiedLog(file).append([entry('b1', null)])
    expect(lines(file)[2]).toMatchObject({ uuid: 'b1', parentUuid: 'a1' })
  })

  it('hangs codex usage rows on the chain without making them the parent of the next entry', () => {
    const dir = tmp()
    const file = path.join(dir, 'L.jsonl')
    const log = new UnifiedLog(file)
    log.append([
      entry('a1', null),
      { type: 'system', subtype: 'api_usage', uuid: 'usage:t1:10', parentUuid: 'a1' } as unknown as JsonlEntry,
      entry('a2', 'a1'),
    ])
    expect(lines(file)[2]).toMatchObject({ uuid: 'a2', parentUuid: 'a1' })
    log.append([
      { type: 'system', subtype: 'api_usage', uuid: 'usage:t2:20', parentUuid: 'a2' } as unknown as JsonlEntry,
    ])
    expect(lastUuidIn(file)).toBe('a2')
  })

  it('shares one writer per file', () => {
    const dir = tmp()
    const file = path.join(dir, 'L.jsonl')
    expect(UnifiedLog.of(file)).toBe(UnifiedLog.of(path.join(dir, '.', 'L.jsonl')))
    UnifiedLog.resetForTests()
  })

  it('keeps the parent of a compact boundary', () => {
    const dir = tmp()
    const file = path.join(dir, 'L.jsonl')
    const log = new UnifiedLog(file)
    log.append([entry('a1', null), { ...entry('cb', null), type: 'system', subtype: 'compact_boundary' } as JsonlEntry])
    expect(lines(file)[1]).toMatchObject({ uuid: 'cb', parentUuid: null })
  })
})

describe('segment pump', () => {
  it('copies complete lines, resumes at its position and follows a compaction successor', () => {
    const root = tmp()
    const native = path.join(root, 'native')
    fs.mkdirSync(native)
    const file = path.join(root, 'L.jsonl')
    const a = path.join(native, 'A.jsonl')
    fs.writeFileSync(a, JSON.stringify(entry('a1', null)) + '\n' + '{"uuid":"a2"')
    const ids: string[] = []
    let at = { file: null as string | null, pos: 0 }
    const pump = new SegmentPump({
      dir: native,
      log: new UnifiedLog(file),
      from: at,
      since: 0,
      onNativeId: (id) => ids.push(id),
      onProgress: (p) => (at = p),
    })
    pump.flush()
    expect(lines(file).map((e) => e.uuid)).toEqual(['a1'])
    fs.appendFileSync(a, ',"parentUuid":"a1","type":"user"}\n')
    pump.flush()
    expect(lines(file).map((e) => e.uuid)).toEqual(['a1', 'a2'])

    // A restarted server continues from the stored position: nothing doubles.
    const again = new SegmentPump({ dir: native, log: new UnifiedLog(file), from: at, since: 0 })
    again.flush()
    expect(lines(file)).toHaveLength(2)

    // Compaction moved the session into a newer file.
    // The successor repeats the history it continues; only b1 is new.
    const b = path.join(native, 'B.jsonl')
    fs.writeFileSync(
      b,
      [entry('a1', null), entry('a2', 'a1'), entry('b1', 'a2')].map((e) => JSON.stringify(e)).join('\n') + '\n',
    )
    const later = new Date(Date.now() + 5_000)
    fs.utimesSync(b, later, later)
    again.flush()
    expect(lines(file).map((e) => e.uuid)).toEqual(['a1', 'a2', 'b1'])
    expect(again.at.file).toBe(b)
    expect(ids).toEqual(['A'])
  })

  it('ignores transcripts older than the segment', () => {
    const root = tmp()
    const old = path.join(root, 'old.jsonl')
    fs.writeFileSync(old, JSON.stringify(entry('o', null)) + '\n')
    const file = path.join(root, 'out', 'L.jsonl')
    new SegmentPump({
      dir: root,
      log: new UnifiedLog(file),
      from: { file: null, pos: 0 },
      since: Date.now() + 60_000,
    }).flush()
    expect(fs.existsSync(file)).toBe(false)
  })
})
