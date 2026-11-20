import fs from 'node:fs'
import fsp from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { indexStats, lineIndex, resetLineIndexForTests, seekLine, STRIDE, tailStart, totalLines } from './line-index'

let dir = ''
beforeEach(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'line-index-'))
  resetLineIndexForTests()
})
afterEach(() => fs.rmSync(dir, { recursive: true, force: true }))

const lineText = (i: number) => JSON.stringify({ i, kind: i % 7 === 0 ? 'keep' : 'skip', pad: 'x'.repeat(i % 50) })

async function withFile<T>(file: string, fn: (fh: fsp.FileHandle) => Promise<T>): Promise<T> {
  const fh = await fsp.open(file, 'r')
  try {
    return await fn(fh)
  } finally {
    await fh.close()
  }
}

describe('sparse line index', () => {
  it('seeks every line and the trailing fragment like a full scan', async () => {
    const file = path.join(dir, 'a.jsonl')
    const n = STRIDE * 3 + 17
    fs.writeFileSync(file, Array.from({ length: n }, (_, i) => lineText(i)).join('\n') + '\n' + '{"torn":')
    await withFile(file, async (fh) => {
      const idx = await lineIndex(file, fh)
      expect(totalLines(idx)).toBe(n + 1)
      for (const i of [0, 1, STRIDE - 1, STRIDE, STRIDE + 1, 2 * STRIDE + 5, n - 1]) {
        expect((await seekLine(fh, idx, i))!.text).toBe(lineText(i))
      }
      expect((await seekLine(fh, idx, n))!.text).toBe('{"torn":')
      expect(await seekLine(fh, idx, n + 1)).toBeNull()
    })
  })

  it('scans only appended bytes and starts over for a rewritten file', async () => {
    const file = path.join(dir, 'b.jsonl')
    fs.writeFileSync(file, Array.from({ length: 3000 }, (_, i) => lineText(i)).join('\n') + '\n')
    const first = fs.statSync(file).size
    await withFile(file, (fh) => lineIndex(file, fh))
    await withFile(file, (fh) => lineIndex(file, fh))
    expect(indexStats).toMatchObject({ builds: 1, extends: 0, bytesScanned: first })
    const more = Array.from({ length: 10 }, (_, i) => lineText(3000 + i)).join('\n') + '\n'
    fs.appendFileSync(file, more)
    await withFile(file, async (fh) => {
      const idx = await lineIndex(file, fh)
      expect(totalLines(idx)).toBe(3010)
      expect((await seekLine(fh, idx, 3005))!.text).toBe(lineText(3005))
    })
    expect(indexStats).toMatchObject({ builds: 1, extends: 1, bytesScanned: first + Buffer.byteLength(more) })
    // A repair cuts the tail: shorter file, index rebuilt.
    fs.writeFileSync(file, lineText(0) + '\n' + lineText(1) + '\n')
    await withFile(file, async (fh) => {
      const idx = await lineIndex(file, fh)
      expect(totalLines(idx)).toBe(2)
      expect((await seekLine(fh, idx, 1))!.text).toBe(lineText(1))
    })
    expect(indexStats.builds).toBe(2)
  })

  it('finds the last N matching lines across index blocks', async () => {
    const file = path.join(dir, 'c.jsonl')
    const n = STRIDE * 4
    fs.writeFileSync(file, Array.from({ length: n }, (_, i) => lineText(i)).join('\n') + '\n')
    const keep = (t: string | null) => t !== null && t.includes('"keep"')
    const expected = Array.from({ length: n }, (_, i) => i).filter((i) => i % 7 === 0)
    await withFile(file, async (fh) => {
      const idx = await lineIndex(file, fh)
      for (const want of [1, 5, 200, 500, expected.length, expected.length + 10]) {
        const line = await tailStart(fh, idx, want, keep)
        expect(line!.index).toBe(expected[Math.max(0, expected.length - want)])
      }
      expect(await tailStart(fh, idx, 3, () => false)).toBeNull()
    })
  })
})
