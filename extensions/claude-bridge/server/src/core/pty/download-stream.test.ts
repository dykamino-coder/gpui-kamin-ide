import { afterEach, describe, expect, it } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { streamFileChunks } from './download-backpressure'

// INC-2026-0078: the transcript download used to read the whole file into one
// string first. It now streams bounded byte chunks under the drain gate.

const dirs: string[] = []
afterEach(() => {
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
})

function file(content: string): { path: string; size: number } {
  const dir = mkdtempSync(join(tmpdir(), 'dl-stream-'))
  dirs.push(dir)
  const path = join(dir, 't.jsonl')
  writeFileSync(path, content)
  return { path, size: Buffer.byteLength(content) }
}

const drain = { limit: 1024, pollMs: 1, timeoutMs: 1000 }

describe('streamFileChunks', () => {
  it('reassembles exactly, with multi-byte characters split across chunk edges', async () => {
    const text = Array.from({ length: 500 }, (_, i) => `{"t":"é😀中 ${i}"}`).join('\n') + '\n'
    const f = file(text)
    const frames: { chunk: string; sent: number; total: number }[] = []
    const ws = { readyState: 1, bufferedAmount: 0, send: (s: string) => frames.push(JSON.parse(s)) }
    // 61-byte chunks (odd, prime) cut through every kind of character.
    await expect(streamFileChunks(ws, f.path, f.size, 61, drain)).resolves.toBeNull()
    expect(frames.map((x) => x.chunk).join('')).toBe(text)
    expect(frames.at(-1)!.sent).toBe(f.size)
    expect(frames.every((x) => x.total === f.size)).toBe(true)
  })

  it('stops on a closed socket instead of reading on', async () => {
    const f = file('x'.repeat(10_000))
    let sends = 0
    const ws = {
      readyState: 1,
      bufferedAmount: 0,
      send: () => {
        if (++sends === 2) ws.readyState = 3
      },
    }
    await expect(streamFileChunks(ws, f.path, f.size, 1000, drain)).resolves.toBe('closed')
    expect(sends).toBe(2)
  })
})
