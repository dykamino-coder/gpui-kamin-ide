import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, describe, expect, it } from 'vitest'
import { lineWindow, readUtf8LineWindow } from './read-range'

// Both ways of cutting a range must number and cut lines exactly as `split('\n')` did.
function reference(text: string, offset: number, limit: number) {
  const all = text.split('\n')
  const lines = offset >= all.length ? [] : all.slice(offset, offset + limit)
  return { lines, more: lines.length > 0 && offset + lines.length < all.length, total: all.length }
}

describe('lineWindow', () => {
  const texts = ['', 'a', 'a\n', 'a\nb', 'a\nb\n', '\n\n\n', 'x\r\ny\r\n', 'one\ntwo\nthree\nfour\nfive']
  for (const text of texts)
    for (const offset of [0, 1, 2, 3, 4, 6])
      for (const limit of [1, 2, 3, 2000])
        it(`${JSON.stringify(text)} offset ${offset} limit ${limit}`, () => {
          const got = lineWindow(text, offset, limit)
          const want = reference(text, offset, limit)
          expect(got.lines).toEqual(want.lines)
          expect(got.more).toBe(want.more)
          expect(got.total()).toBe(want.total)
        })
})

describe('readUtf8LineWindow', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'read-range-'))
  afterAll(() => fs.rmSync(dir, { recursive: true, force: true }))
  const file = (name: string, data: string | Buffer) => {
    const p = path.join(dir, name)
    fs.writeFileSync(p, data)
    return p
  }
  // Multi-byte characters land on the 256 KiB chunk boundaries.
  const big = Array.from({ length: 30_000 }, (_, i) => `строка ${i} ✓ ${'ж'.repeat(i % 7)}`).join('\n') + '\n'
  const cases: [string, string, Buffer][] = [
    ['big', big, Buffer.from(big)],
    ['big-bom', big, Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from(big)])],
    ['crlf', big.replaceAll('\n', '\r\n'), Buffer.from(big.replaceAll('\n', '\r\n'))],
    ['no-final-newline', 'a\nb\nc', Buffer.from('a\nb\nc')],
  ]
  for (const [name, text, bytes] of cases)
    for (const [offset, limit] of [[0, 3], [14_999, 50], [29_998, 5], [30_000, 5], [40_000, 5], [1, 2000]] as const)
      for (const countAll of [true, false])
        it(`${name} offset ${offset} limit ${limit} countAll ${countAll}`, async () => {
          const got = (await readUtf8LineWindow(file(name, bytes), offset, limit, countAll))!
          const want = reference(text, offset, limit)
          expect(got.lines).toEqual(want.lines)
          expect(got.more).toBe(want.more)
          if (countAll || !got.more) expect(got.total()).toBe(want.total)
        })

  it('leaves UTF-16 and binary files to the whole-file path', async () => {
    const utf16 = Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from('a\nb\n', 'utf16le')])
    expect(await readUtf8LineWindow(file('u16', utf16), 0, 2, true)).toBeNull()
    expect(await readUtf8LineWindow(file('bin', Buffer.from([65, 0, 66, 10])), 0, 2, true)).toBeNull()
  })
})
