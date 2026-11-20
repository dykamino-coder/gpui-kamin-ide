import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { lastUuidIn, seedUnifiedLog, UnifiedLog } from './unified-log'

let dir = ''
beforeEach(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'unified-scan-'))
  UnifiedLog.resetForTests()
})
afterEach(() => fs.rmSync(dir, { recursive: true, force: true }))

const row = (uuid: string, extra: Record<string, unknown> = {}, pad = 0) =>
  JSON.stringify({ type: 'assistant', uuid, message: { content: 'ж'.repeat(pad) }, ...extra })

describe('unified log scans read in chunks', () => {
  it('finds the last main-chain uuid behind side chains, usage rows and lines over a chunk', () => {
    const file = path.join(dir, 'a.jsonl')
    fs.writeFileSync(
      file,
      [
        row('first'),
        row('main', {}, 700_000), // 1.4 MB: spans chunk boundaries
        row('side', { isSidechain: true }, 900_000),
        JSON.stringify({ type: 'system', subtype: 'api_usage', uuid: 'usage' }),
        '{"torn":',
      ].join('\n'),
    )
    expect(lastUuidIn(file)).toBe('main')
    expect(lastUuidIn(path.join(dir, 'missing.jsonl'))).toBeNull()
    fs.writeFileSync(file, row('only'))
    expect(lastUuidIn(file)).toBe('only')
  })

  it('knows every uuid of a log larger than a chunk and appends none twice', () => {
    const file = path.join(dir, 'b.jsonl')
    const ids = Array.from({ length: 300 }, (_, i) => `id-${i}`)
    fs.writeFileSync(file, ids.map((id, i) => row(id, { parentUuid: i ? ids[i - 1] : null }, 4_000)).join('\n') + '\n')
    expect(fs.statSync(file).size).toBeGreaterThan(2 * 1024 * 1024)
    const log = UnifiedLog.of(file)
    const before = fs.statSync(file).size
    log.append(ids.map((id) => JSON.parse(row(id))))
    expect(fs.statSync(file).size).toBe(before)
    log.append([JSON.parse(row('new'))])
    const last = JSON.parse(fs.readFileSync(file, 'utf8').trimEnd().split('\n').pop()!)
    expect(last).toMatchObject({ uuid: 'new', parentUuid: 'id-299' })
  })

  it('seeds a byte copy without the torn last line', () => {
    const source = path.join(dir, 'src.jsonl')
    const log = path.join(dir, 'log', 'c.jsonl')
    const whole = `${row('a', {}, 600_000)}\n${row('b')}\n`
    fs.writeFileSync(source, `${whole}{"torn":`)
    seedUnifiedLog(log, source)
    expect(fs.readFileSync(log, 'utf8')).toBe(whole)
    fs.writeFileSync(source, '{"no newline":')
    seedUnifiedLog(log, source)
    expect(fs.readFileSync(log, 'utf8')).toBe('')
    seedUnifiedLog(log, path.join(dir, 'missing.jsonl'))
    expect(fs.readFileSync(log, 'utf8')).toBe('')
  })
})
