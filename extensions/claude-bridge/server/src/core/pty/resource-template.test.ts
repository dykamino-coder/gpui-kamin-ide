import { describe, expect, it } from 'vitest'
import { matchesResourceTemplate } from './resource-template'

describe('matchesResourceTemplate', () => {
  it('matches expanded templates without interpreting or rewriting their values', () => {
    expect(matchesResourceTemplate('repo://{owner}/{name}/file/{path}', 'repo://openai/codex/file/src/main.ts')).toBe(true)
    expect(matchesResourceTemplate('search://items{?query,limit}', 'search://items?query=hooks&limit=5')).toBe(true)
    expect(matchesResourceTemplate('repo://{owner}/{name}', 'other://openai/codex')).toBe(false)
  })

  it('requires exact equality when there are no expressions and rejects malformed templates', () => {
    expect(matchesResourceTemplate('file:///readme.md', 'file:///readme.md')).toBe(true)
    expect(matchesResourceTemplate('file:///readme.md', 'file:///other.md')).toBe(false)
    expect(matchesResourceTemplate('file:///{path', 'file:///readme.md')).toBe(false)
    expect(matchesResourceTemplate('file:///{}', 'file:///readme.md')).toBe(false)
  })
})
