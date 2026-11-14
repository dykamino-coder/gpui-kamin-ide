import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { editFile, globSearch, readFile, writeFile } from './file-system'
import { clearFileCache } from '../../utils/file-read-cache'

// Read/Write/Edit must never distort a file silently: ambiguous edits fail like
// native Edit, encodings and line endings survive, failures are MCP errors.
let dir = ''
beforeEach(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fs-quality-'))
  clearFileCache()
})
afterEach(() => fs.rmSync(dir, { recursive: true, force: true }))

const text = (r: { content: Array<{ text?: string }> }): string => r.content.map((c) => c.text ?? '').join('')
const file = (name: string, data: string | Buffer): string => {
  const p = path.join(dir, name)
  fs.writeFileSync(p, data)
  return p
}

describe('Edit', () => {
  it('refuses an ambiguous old_string unless replace_all', async () => {
    const p = file('dup.ts', 'a = 1\na = 1\n')
    const r = await editFile({ file_path: p, old_string: 'a = 1', new_string: 'a = 2' })
    expect(r.isError).toBe(true)
    expect(text(r)).toContain('Found 2 matches')
    expect(fs.readFileSync(p, 'utf8')).toBe('a = 1\na = 1\n')
    const all = await editFile({ file_path: p, old_string: 'a = 1', new_string: 'a = 2', replace_all: true })
    expect(all.isError).toBeUndefined()
    expect(fs.readFileSync(p, 'utf8')).toBe('a = 2\na = 2\n')
  })

  it('refuses an empty old_string on a non-empty file and creates a missing one', async () => {
    const p = file('full.ts', 'keep\n')
    const r = await editFile({ file_path: p, old_string: '', new_string: 'x' })
    expect(r.isError).toBe(true)
    expect(fs.readFileSync(p, 'utf8')).toBe('keep\n')
    const created = path.join(dir, 'sub', 'new.ts')
    expect((await editFile({ file_path: created, old_string: '', new_string: 'hello\n' })).isError).toBeUndefined()
    expect(fs.readFileSync(created, 'utf8')).toBe('hello\n')
  })

  it('reports a missing string as an error', async () => {
    const p = file('a.ts', 'abc\n')
    const r = await editFile({ file_path: p, old_string: 'zzz', new_string: 'y' })
    expect(r.isError).toBe(true)
    expect(text(r)).toContain('String to replace not found')
  })

  it('edits a CRLF file with LF old/new text and keeps every other byte', async () => {
    const p = file('crlf.ts', 'one\r\ntwo\r\nthree\nfour\r\n')
    const r = await editFile({ file_path: p, old_string: 'one\ntwo', new_string: 'one\nTWO\nextra' })
    expect(r.isError).toBeUndefined()
    // The mixed LF line stays LF: no whole-file line-ending rewrite.
    expect(fs.readFileSync(p, 'utf8')).toBe('one\r\nTWO\r\nextra\r\nthree\nfour\r\n')
  })

  it('keeps UTF-16 LE with BOM and UTF-8 BOM encodings', async () => {
    const u16 = file('u16.txt', Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from('héllo\r\nмир\r\n', 'utf16le')]))
    expect((await editFile({ file_path: u16, old_string: 'мир', new_string: 'мир!' })).isError).toBeUndefined()
    const bytes = fs.readFileSync(u16)
    expect([...bytes.subarray(0, 2)]).toEqual([0xff, 0xfe])
    expect(bytes.subarray(2).toString('utf16le')).toBe('héllo\r\nмир!\r\n')

    const bom = file('bom.txt', '﻿a\n')
    expect((await editFile({ file_path: bom, old_string: 'a', new_string: 'b' })).isError).toBeUndefined()
    expect(fs.readFileSync(bom, 'utf8')).toBe('﻿b\n')
  })

  it('removes the line ending with a deleted whole line', async () => {
    const p = file('lines.ts', 'a\nb\nc\n')
    await editFile({ file_path: p, old_string: 'b', new_string: '' })
    expect(fs.readFileSync(p, 'utf8')).toBe('a\nc\n')
  })
})

describe('Read', () => {
  it('decodes UTF-16 and strips CR from CRLF lines', async () => {
    const p = file('u16.txt', Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from('héllo\r\nмир\r\n', 'utf16le')]))
    expect(text(await readFile({ file_path: p }))).toBe('     1\théllo\n     2\tмир')
  })

  it('refuses binary files and reports missing files as errors', async () => {
    const bin = file('blob.bin', Buffer.from([0, 1, 2, 3, 0xff, 0]))
    const r = await readFile({ file_path: bin })
    expect(r.isError).toBe(true)
    expect(text(r)).toContain('binary')
    const missing = await readFile({ file_path: path.join(dir, 'nope.txt') })
    expect(missing.isError).toBe(true)
    expect(text(missing)).toContain('does not exist')
  })

  it('says when the offset is past the end and when lines were left out', async () => {
    const p = file('lines.txt', Array.from({ length: 2500 }, (_, i) => `l${i}`).join('\n'))
    expect(text(await readFile({ file_path: p, offset: 3000 }))).toContain('shorter than the provided offset')
    expect(text(await readFile({ file_path: p }))).toContain('Showing lines 1-2000 of 2500')
  })
})

describe('Write', () => {
  it('keeps the encoding of the file it replaces', async () => {
    const p = file('u16.txt', Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from('old', 'utf16le')]))
    await writeFile({ file_path: p, content: 'new' })
    expect(fs.readFileSync(p)).toEqual(Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from('new', 'utf16le')]))
  })
})

describe('Glob', () => {
  it('returns at most 100 paths and says how many matched', async () => {
    for (let i = 0; i < 120; i++) file(`f${i}.txt`, '')
    const out = text(await globSearch({ pattern: '*.txt', path: dir }))
    expect(out.split('\n').filter((l) => l.endsWith('.txt'))).toHaveLength(100)
    expect(out).toContain('of 120 matching files')
  })
})
