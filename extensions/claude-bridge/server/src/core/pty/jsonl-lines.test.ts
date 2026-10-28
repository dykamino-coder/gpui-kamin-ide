import { afterEach, describe, expect, it } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { readJsonlLines, type JsonlLine } from './jsonl-lines'

const dirs: string[] = []
afterEach(() => {
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
})

function file(content: string | Buffer): string {
  const dir = mkdtempSync(join(tmpdir(), 'jsonl-lines-'))
  dirs.push(dir)
  const p = join(dir, 't.jsonl')
  writeFileSync(p, content)
  return p
}

async function all(p: string, opts: Parameters<typeof readJsonlLines>[1] = {}): Promise<JsonlLine[]> {
  const out: JsonlLine[] = []
  for await (const l of readJsonlLines(p, opts)) out.push(l)
  return out
}

describe('readJsonlLines — byte framing', () => {
  it('keeps multi-byte characters and exact offsets on every chunk boundary', async () => {
    const lines = ['{"t":"a😀b"}', '{"t":"кириллица"}', '{"t":"中文字符"}', '{"t":"plain"}']
    const content = lines.join('\n') + '\n'
    const bytes = Buffer.from(content, 'utf8')
    const p = file(bytes)
    // Every chunk size from 1 byte up cuts some character in the middle.
    for (let chunk = 1; chunk <= 24; chunk++) {
      const got = await all(p, { chunkBytes: chunk })
      expect(got.map((l) => l.text)).toEqual(lines)
      let pos = 0
      for (const l of got) {
        expect(l.start).toBe(pos)
        expect(bytes.toString('utf8', l.start, l.end)).toBe(l.text + '\n')
        pos = l.end
      }
      expect(pos).toBe(bytes.length)
    }
  })

  it('reports an unterminated last line and tolerates CRLF and blank lines', async () => {
    const p = file('{"a":1}\r\n\n{"b":2}\n{"c":')
    const got = await all(p, { chunkBytes: 3 })
    expect(got.map((l) => [l.text, l.terminated])).toEqual([
      ['{"a":1}', true],
      ['{"b":2}', true],
      ['{"c":', false],
    ])
    expect(got[2]!.end).toBe(Buffer.byteLength('{"a":1}\r\n\n{"b":2}\n{"c":'))
  })

  it('reads only the requested range', async () => {
    const p = file('{"a":1}\n{"b":2}\n{"c":3}\n')
    const got = await all(p, { start: 8, end: 16 })
    expect(got.map((l) => l.text)).toEqual(['{"b":2}'])
  })

  it('stops reading once aborted — no further line is produced', async () => {
    const rows = Array.from({ length: 5000 }, (_, i) => JSON.stringify({ i, pad: 'x'.repeat(200) }))
    const p = file(rows.join('\n') + '\n')
    const ac = new AbortController()
    let seen = 0
    await expect(
      (async () => {
        for await (const _ of readJsonlLines(p, { signal: ac.signal, chunkBytes: 4096 })) {
          seen++
          if (seen === 10) ac.abort()
        }
      })(),
    ).rejects.toThrow()
    expect(seen).toBe(10)
  })
})
