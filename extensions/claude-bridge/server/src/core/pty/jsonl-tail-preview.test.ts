import { describe, it, expect, afterEach } from 'vitest'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'
import type { JsonlEntry } from '../../shared/jsonl-types'

type Tagged = JsonlEntry & { _pos?: number; _posEnd?: number; _ord?: number }

/** A transcript comfortably past the 512KB preview window. */
function bigTranscript(file: string): { lines: number } {
  const rows: string[] = []
  let n = 0
  while (rows.join('\n').length < 900_000) {
    n++
    rows.push(JSON.stringify({
      type: n % 2 ? 'user' : 'assistant',
      uuid: `u-${String(n)}`,
      timestamp: new Date(Date.UTC(2026, 0, 1, 0, 0, n)).toISOString(),
      message: { role: n % 2 ? 'user' : 'assistant', content: 'x'.repeat(400) },
    }))
    // Every 10th row is uuid-less bookkeeping — the kind the preview must skip.
    if (n % 10 === 0) rows.push(JSON.stringify({ type: 'system', subtype: 'turn_duration', content: '' }))
  }
  writeFileSync(file, rows.join('\n') + '\n')
  return { lines: rows.length }
}

describe('JsonlWatcher — tail preview', () => {
  const dirs: string[] = []
  const watchers: JsonlWatcher[] = []
  afterEach(() => {
    for (const w of watchers.splice(0)) w.stop()
    for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
  })

  function setup(): { w: JsonlWatcher; batches: Tagged[][] } {
    const dir = mkdtempSync(join(tmpdir(), 'jsonl-preview-'))
    dirs.push(dir)
    const file = join(dir, 'big.jsonl')
    bigTranscript(file)
    const batches: Tagged[][] = []
    const w = new JsonlWatcher(Date.now(), (es) => { batches.push(es as Tagged[]); return true }, () => {})
    watchers.push(w)
    ;(w as unknown as { filePath: string }).filePath = file
    return { w, batches }
  }

  it('delivers the newest records before the full parse finishes', async () => {
    const { w, batches } = setup()
    w.replayAll()
    // Far less than a full parse of ~900KB would take.
    await new Promise((r) => setTimeout(r, 120))
    expect(batches.length).toBeGreaterThan(0)
    const first = batches[0]!
    expect(first.length).toBeGreaterThan(0)
    // The tail, not the head.
    const nums = first.map((e) => Number(String(e.uuid).slice(2)))
    expect(Math.min(...nums)).toBeGreaterThan(100)
  })

  it('previews only uuid-bearing records', async () => {
    // A uuid-less row is deduped by `_ord` alone, and the authoritative pass
    // numbers `_ord` differently — previewing one would duplicate it on screen.
    const { w, batches } = setup()
    w.replayAll()
    await new Promise((r) => setTimeout(r, 120))
    expect(batches[0]!.every((e) => typeof e.uuid === 'string' && e.uuid.length > 0)).toBe(true)
  })

  it('tags exact byte offsets so a mirrored preview record stays valid', async () => {
    const { w, batches } = setup()
    w.replayAll()
    await new Promise((r) => setTimeout(r, 120))
    for (const e of batches[0]!) {
      expect(typeof e._pos).toBe('number')
      expect(e._posEnd).toBeGreaterThan(e._pos!)
    }
  })

  it('numbers the full pass strictly above every preview _ord', async () => {
    // A collision would make the client's `_ord` dedup drop a DIFFERENT record
    // that was never displayed.
    const { w, batches } = setup()
    w.replayAll()
    await new Promise((r) => setTimeout(r, 2500))
    expect(batches.length).toBeGreaterThan(1)
    const previewMax = Math.max(...batches[0]!.map((e) => e._ord ?? -1))
    const laterOrds = batches.slice(1).flat().map((e) => e._ord ?? -1).filter((o) => o >= 0)
    expect(laterOrds.length).toBeGreaterThan(0)
    expect(Math.min(...laterOrds)).toBeGreaterThan(previewMax)
  })
})
