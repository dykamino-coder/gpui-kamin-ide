import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { readHandoff, type HandoffSource } from './handoff'
import {
  copyDigest,
  digestOf,
  digestStats,
  forgetDigest,
  handoffFromLog,
  resetDigestsForTests,
  setCheckpointBytesForTests,
  sidecarOf,
} from './transcript-digest'

const source: HandoffSource = { conversationId: 'c1', engine: 'claude', model: 'm', target: 'codex' }

let seed = 7
const rnd = () => (seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff
const words = ['alpha', 'бета', '数据', 'delta', 'x'.repeat(40), 'line\nbreak', '']
const text = (n: number) => Array.from({ length: n }, () => words[Math.floor(rnd() * words.length)]).join(' ')

function entries(n: number, from = 0): object[] {
  const out: object[] = []
  for (let i = from; i < from + n; i++) {
    const at = new Date(Date.UTC(2026, 9, 1, 0, 0, 0) + i * 7 * 60_000).toISOString()
    const size = rnd() < 0.05 ? 1500 : rnd() < 0.3 ? 120 : 10
    const r = rnd()
    const base = { uuid: `u-${i}`, timestamp: at }
    if (r < 0.35) out.push({ ...base, type: 'user', message: { role: 'user', content: text(size) } })
    else if (r < 0.45)
      out.push({
        ...base,
        type: 'user',
        message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: `t${i - 1}`, content: text(size) }] },
      })
    else if (r < 0.48)
      out.push({
        ...base,
        type: 'system',
        subtype: 'compact_boundary',
        compactMetadata: { trigger: 'auto', preTokens: 1000 },
      })
    else if (r < 0.5) out.push({ ...base, type: 'system', subtype: 'engine_switch', engine: 'codex', model: 'gpt' })
    else
      out.push({
        ...base,
        type: 'assistant',
        message: {
          role: 'assistant',
          content: [
            { type: 'text', text: text(size) },
            ...(rnd() < 0.5 ? [{ type: 'tool_use', id: `t${i}`, name: 'Read', input: { p: text(3) } }] : []),
          ],
        },
      })
  }
  return out
}

const lines = (es: object[]) => es.map((e) => JSON.stringify(e) + '\n').join('')

let dir: string
let file: string
beforeEach(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'digest-'))
  file = path.join(dir, 'c1.jsonl')
  resetDigestsForTests()
  setCheckpointBytesForTests(16 * 1024)
})
afterEach(() => {
  resetDigestsForTests()
  setCheckpointBytesForTests(4 * 1024 * 1024)
  fs.rmSync(dir, { recursive: true, force: true })
})

describe('conversation digest', { timeout: 30_000 }, () => {
  it('gives the whole-log handoff while the log grows, across restarts', async () => {
    let n = 0
    for (let step = 0; step < 25; step++) {
      const more = 1 + Math.floor(rnd() * 60)
      const add = lines(entries(more, n))
      n += more
      if (rnd() < 0.3) resetDigestsForTests() // a server restart
      // One sidecar per budget: a small one exercises the shortened block.
      for (const budgetBytes of [3_000, 64_000]) {
        const log = path.join(dir, `b${budgetBytes}.jsonl`)
        fs.appendFileSync(log, add)
        expect(await handoffFromLog(log, source, { budgetBytes })).toEqual(
          await readHandoff(log, source, { budgetBytes }),
        )
      }
    }
    expect(digestStats.resumes).toBeGreaterThan(0)
  })

  it('a restart reads only the lines after the last checkpoint', async () => {
    fs.writeFileSync(file, lines(entries(2000)))
    await digestOf(file)
    const size = fs.statSync(file).size
    resetDigestsForTests()
    await digestOf(file)
    expect(digestStats.builds).toBe(0)
    expect(digestStats.resumes).toBe(1)
    expect(digestStats.bytesParsed).toBeLessThan(size / 10)
  })

  it('a torn last line counts in the block but is not stored', async () => {
    fs.writeFileSync(file, lines(entries(50)) + JSON.stringify(entries(1, 50)[0]))
    expect(await handoffFromLog(file, source)).toEqual(await readHandoff(file, source))
    const d = await digestOf(file)
    expect(d!.lines).toBe(50)
  })

  it('a torn sidecar record, a cut log tail and a rewritten middle resume from what still holds', async () => {
    const es = entries(1500)
    fs.writeFileSync(file, lines(es))
    await digestOf(file)
    // Crash mid-record.
    fs.appendFileSync(sidecarOf(file), '{"k":"u","line":')
    resetDigestsForTests()
    expect(await handoffFromLog(file, source)).toEqual(await readHandoff(file, source))
    expect(digestStats.builds).toBe(0)
    // The log loses its last lines (repair cuts a torn tail).
    const kept = lines(es.slice(0, 1400))
    fs.writeFileSync(file, kept)
    resetDigestsForTests()
    expect(await handoffFromLog(file, source)).toEqual(await readHandoff(file, source))
    expect(digestStats.builds).toBe(0)
    // A line in the second half is rewritten in place (same process).
    const changed = es.slice(0, 1400).map((e, i) => (i === 1000 ? { ...e, uuid: 'rewritten' } : e))
    fs.writeFileSync(file, lines(changed))
    expect(await handoffFromLog(file, source)).toEqual(await readHandoff(file, source))
    expect(digestStats.builds).toBe(0)
    const d = await digestOf(file)
    expect(d!.lines).toBe(1400)
    expect(d!.requests.find((r) => r.line === 1000)?.uuid ?? 'rewritten').toBe('rewritten')
  })

  it('a log replaced by other content starts over', async () => {
    fs.writeFileSync(file, lines(entries(300)))
    await digestOf(file)
    fs.writeFileSync(file, lines(entries(300, 5000)))
    resetDigestsForTests()
    expect(await handoffFromLog(file, source)).toEqual(await readHandoff(file, source))
    expect(digestStats.builds).toBe(1)
  })

  it('a seeded copy takes over the sidecar', async () => {
    fs.writeFileSync(file, lines(entries(1200)))
    await digestOf(file)
    const copy = path.join(dir, 'unified.jsonl')
    fs.copyFileSync(file, copy)
    copyDigest(file, copy)
    resetDigestsForTests()
    expect(await handoffFromLog(copy, source)).toEqual(await readHandoff(copy, source))
    expect(digestStats.builds).toBe(0)
  })

  it('points at every request, compaction and engine switch, and counts hours', async () => {
    const es = entries(400) as Array<Record<string, unknown>>
    fs.writeFileSync(file, lines(es))
    const d = (await digestOf(file))!
    const users = es.flatMap((e, line) =>
      e.type === 'user' &&
      typeof (e.message as { content: unknown }).content === 'string' &&
      String((e.message as { content: string }).content).trim()
        ? [line]
        : [],
    )
    expect(d.requests.map((r) => r.line)).toEqual(users)
    const offsets = [0]
    for (const e of es) offsets.push(offsets[offsets.length - 1]! + Buffer.byteLength(JSON.stringify(e)) + 1)
    for (const r of [...d.requests, ...d.compactions, ...d.segments]) {
      expect(r.offset).toBe(offsets[r.line])
      expect(r.uuid).toBe(`u-${r.line}`)
    }
    expect(d.compactions.map((c) => c.line)).toEqual(
      es.flatMap((e, i) => (e.subtype === 'compact_boundary' ? [i] : [])),
    )
    expect(d.segments.map((c) => c.line)).toEqual(es.flatMap((e, i) => (e.subtype === 'engine_switch' ? [i] : [])))
    const hours = [...d.periods, d.period!]
    expect(hours.reduce((n, h) => n + h.bytes, 0)).toBe(fs.statSync(file).size)
    expect(hours.reduce((n, h) => n + h.turns, 0)).toBe(d.handoff.total)
    expect(new Set(hours.map((h) => h.hour)).size).toBe(hours.length)
  })

  it('the note points at the line the verbatim history starts on', async () => {
    fs.writeFileSync(file, lines(entries(3000)))
    const h = (await handoffFromLog(file, source))!
    const m = /read the lines before line (\d+) \(entry (u-\d+)\) with from_index/.exec(h.text)
    expect(m).not.toBeNull()
    expect(m![2]).toBe(`u-${m![1]}`)
  })

  it('the line index of a restarted process comes from the sidecar, without a scan', async () => {
    const { lineIndex, indexStats, resetLineIndexForTests, totalLines } = await import('../project-sessions/line-index')
    fs.writeFileSync(file, lines(entries(3000)))
    await digestOf(file)
    resetDigestsForTests()
    resetLineIndexForTests()
    const fh = await fs.promises.open(file, 'r')
    try {
      const idx = await lineIndex(file, fh)
      expect(indexStats.fromDigest).toBe(1)
      expect(indexStats.bytesScanned).toBe(0)
      expect(totalLines(idx)).toBe(3000)
      expect(idx.offsets.length).toBe(3)
    } finally {
      await fh.close()
    }
  })

  it('a deleted log takes its sidecar with it', async () => {
    fs.writeFileSync(file, lines(entries(10)))
    await digestOf(file)
    expect(fs.existsSync(sidecarOf(file))).toBe(true)
    fs.rmSync(file)
    forgetDigest(file)
    expect(fs.existsSync(sidecarOf(file))).toBe(false)
    fs.writeFileSync(file, lines(entries(10)))
    await digestOf(file)
    fs.rmSync(file)
    expect(await digestOf(file)).toBeNull()
    expect(fs.existsSync(sidecarOf(file))).toBe(false)
  })
})
