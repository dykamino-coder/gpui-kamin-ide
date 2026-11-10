import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fallbackResult, walkFallback } from './grep-tool'

// The no-ripgrep fallback must tell the model when its deadline cut the walk
// short, instead of returning a partial list or "No matches found." as if the
// search had completed (the class of bug Claude Code 2.1.317 fixed natively).
let dir = ''
beforeEach(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'grep-fallback-'))
  for (const name of ['a.txt', 'b.txt', 'c.txt']) fs.writeFileSync(path.join(dir, name), 'needle\n')
})
afterEach(() => {
  vi.restoreAllMocks()
  fs.rmSync(dir, { recursive: true, force: true })
})

const text = (r: { content: Array<{ text?: string }> }): string => r.content.map((c) => c.text ?? '').join('')

describe('grep fallback deadline', () => {
  it('completes within budget without a timeout notice', async () => {
    const walk = await walkFallback(/needle/g, dir, 'files_with_matches', 0, 60_000)
    expect(walk).toMatchObject({ timedOut: false })
    expect(walk.results).toHaveLength(3)
    const r = fallbackResult(walk.results, 0, 0, walk.timedOut, 60_000)
    expect(text(r)).not.toMatch(/^Error:/)
    expect(text(r)).not.toContain('timed out')
  })

  it('reports a timeout instead of "No matches found." when nothing was searched', async () => {
    const walk = await walkFallback(/needle/g, dir, 'files_with_matches', 0, -1)
    expect(walk).toEqual({ results: [], timedOut: true })
    const r = fallbackResult(walk.results, 0, 0, walk.timedOut, 540_000)
    expect(text(r)).toMatch(/^Error:/)
    expect(text(r)).toContain('timed out after 540s')
    expect(text(r)).not.toContain('No matches found.')
  })

  it('marks results as partial when the deadline passes mid-walk', async () => {
    const start = Date.now()
    let calls = 0
    // The deadline and the first entry check are inside the budget, later checks are past it.
    vi.spyOn(Date, 'now').mockImplementation(() => (calls++ < 2 ? start : start + 10_000))
    const walk = await walkFallback(/needle/g, dir, 'files_with_matches', 0, 1_000)
    expect(walk.timedOut).toBe(true)
    expect(walk.results).toHaveLength(1)
    const r = fallbackResult(walk.results, 0, 0, walk.timedOut, 1_000)
    expect(text(r)).not.toMatch(/^Error:/)
    expect(text(r)).toContain('partial results above')
  })

  it('keeps a complete empty search as "No matches found."', () => {
    expect(text(fallbackResult([], 0, 0, false, 540_000))).toBe('No matches found.')
  })
})
