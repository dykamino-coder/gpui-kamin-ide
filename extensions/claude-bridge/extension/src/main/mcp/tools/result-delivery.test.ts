import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { readFile } from './file-system'
import { fallbackResult, grepSearch, paginationNotice } from './grep-tool'
import { HEAD_BYTES, INLINE_BYTES, StreamCapture, TAIL_BYTES } from './shell-output'
import { clearFileCache } from '../../utils/file-read-cache'

// INC-2026-0121: a result the model cannot use whole must say so and keep the
// rest reachable on this machine — never a silent cut, never a file the
// client-side Read cannot open.
let dir = ''
beforeEach(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'result-delivery-'))
  clearFileCache()
})
afterEach(() => fs.rmSync(dir, { recursive: true, force: true }))

const text = (r: { content: Array<{ text?: string }> }): string => r.content.map((c) => c.text ?? '').join('')
const line = (i: number): string => `export const value${i} = 'текст' // ${'x'.repeat(60)}\n`

describe('Read size limits', () => {
  it('refuses a whole-file read above 256 KB with a hint, like native Read', async () => {
    const p = path.join(dir, 'big.ts')
    fs.writeFileSync(p, Array.from({ length: 4000 }, (_, i) => line(i)).join(''))
    const r = await readFile({ file_path: p })
    expect(r.isError).toBe(true)
    expect(text(r)).toMatch(/^Error: File content \(\d+KB\) exceeds the maximum size for a whole-file read \(256KB\)\. Use offset and limit/)
  })

  it('still reads a range of the same file', async () => {
    const p = path.join(dir, 'big.ts')
    fs.writeFileSync(p, Array.from({ length: 4000 }, (_, i) => line(i)).join(''))
    const r = await readFile({ file_path: p, offset: 3000, limit: 2 })
    expect(r.isError).toBeFalsy()
    expect(text(r)).toContain('value2999')
    expect(text(r)).toContain('value3000')
  })

  it('refuses a range whose text exceeds 256 KB', async () => {
    const p = path.join(dir, 'wide.txt')
    fs.writeFileSync(p, Array.from({ length: 200 }, () => 'w'.repeat(1900) + '\n').join(''))
    const r = await readFile({ file_path: p, offset: 1, limit: 200 })
    expect(r.isError).toBe(true)
    expect(text(r)).toContain('Read fewer lines with a smaller limit')
  })
})

describe('shell output capture', () => {
  it('returns small output unchanged and writes no file', () => {
    const out = path.join(dir, 'out')
    const c = new StreamCapture('stdout', out)
    c.push(Buffer.from('hello\n'))
    expect(c.finish()).toBe('hello\n')
    expect(fs.existsSync(out)).toBe(false)
  })

  it('keeps head and tail inline and the whole stream in a file on this machine', () => {
    const out = path.join(dir, 'out')
    const c = new StreamCapture('stdout', out)
    const lines = Array.from({ length: 200_000 }, (_, i) => `${i + 1}\n`).join('')
    const bytes = Buffer.from(lines)
    for (let i = 0; i < bytes.length; i += 65_536) c.push(bytes.subarray(i, i + 65_536))
    const result = c.finish()
    expect(result.startsWith('1\n2\n3\n')).toBe(true)
    expect(result).toContain('199999\n200000\n')
    const saved = /Full output saved to (.+?); read it with Read/.exec(result)![1]!
    expect(path.dirname(saved)).toBe(out)
    expect(fs.readFileSync(saved, 'utf8')).toBe(lines)
    expect(result.length).toBeLessThan(HEAD_BYTES + TAIL_BYTES + 1024)
  })

  it('never splits a UTF-8 character at the head or tail boundary', () => {
    const c = new StreamCapture('stdout', path.join(dir, 'out'))
    c.push(Buffer.from('ж'.repeat(INLINE_BYTES)))
    const result = c.finish()
    expect(result).not.toContain('�')
  })
})

describe('Grep head_limit marker', () => {
  it('marks a cut result like native Grep and leaves an exact fit unmarked', () => {
    expect(paginationNotice(51, 0, 50)).toBe('\n\n[Showing results with pagination = limit: 50]')
    expect(paginationNotice(50, 0, 50)).toBe('')
    expect(paginationNotice(100, 10, 20)).toBe('\n\n[Showing results with pagination = limit: 20, offset: 10]')
    expect(paginationNotice(100, 0, 0)).toBe('')
  })

  it('marks a head_limit cut of a real search (ripgrep or the fallback, whichever this host has)', async () => {
    fs.writeFileSync(path.join(dir, 'm.txt'), 'hit 1\nhit 2\nhit 3\n')
    const content = await grepSearch({ pattern: 'hit', path: dir, output_mode: 'content', head_limit: 2 })
    expect(text(content)).toMatch(/hit 2\n\n\[Showing results with pagination = limit: 2\]/)
    const exact = await grepSearch({ pattern: 'hit', path: dir, output_mode: 'content', head_limit: 3 })
    expect(text(exact)).not.toContain('pagination')
    const offset = await grepSearch({ pattern: 'hit', path: dir, output_mode: 'content', head_limit: 1, offset: 1 })
    expect(text(offset)).toContain('[Showing results with pagination = limit: 1, offset: 1]')
  })

  it('marks the fallback result too', () => {
    const r = fallbackResult(['a', 'b', 'c'], 0, 2, false, 1000)
    expect(text(r)).toBe('a\nb\n\n[Showing results with pagination = limit: 2]')
  })
})
