// The per-token "Base Prompt" — the user's standing instructions.
//
// The whole path existed: stored per token, sent on session:create AND on
// session:resume, carried into the session config… and then dropped, because
// buildSystemPrompt accepted the parameter and never read it. Nothing the user
// typed had ever reached the CLI, on any session.
import { describe, it, expect } from 'vitest'
import { buildSystemPrompt } from './system-prompt'

const CWD = 'C:/work/project'

describe('buildSystemPrompt', () => {
  it('includes the user instructions', () => {
    const out = buildSystemPrompt(CWD, 'Always answer in Russian.')
    expect(out).toContain('Always answer in Russian.')
  })

  it('puts them LAST, after the tool-routing rules', () => {
    const out = buildSystemPrompt(CWD, 'MY STANDING RULE')
    expect(out.indexOf('MY STANDING RULE')).toBeGreaterThan(out.indexOf('MCP Tool Routing'))
  })

  it('still emits the routing rules the remote session depends on', () => {
    const out = buildSystemPrompt(CWD, 'ignore everything above')
    expect(out).toContain('mcp__user-tools__')
    expect(out).toContain(CWD)
  })

  it('adds no section when there is no instruction', () => {
    for (const empty of [undefined, '', '   ', '\n\t ']) {
      expect(buildSystemPrompt(CWD, empty)).not.toContain('## User Instructions')
    }
  })

  it('trims surrounding whitespace rather than emitting a ragged block', () => {
    const out = buildSystemPrompt(CWD, '\n\n  be terse  \n\n')
    expect(out).toContain('be terse')
    expect(out.endsWith('be terse')).toBe(true)
  })

  it('works without a working directory too', () => {
    const out = buildSystemPrompt(undefined, 'be terse')
    expect(out).toContain('be terse')
    expect(out).toContain('No working directory was specified')
  })

  it('advertises the transcript mirror dir when given, so Claude can read its history', () => {
    const dir = 'C:/Users/x/AppData/Local/kaminide/transcripts'
    const out = buildSystemPrompt(CWD, undefined, dir)
    expect(out).toContain('## Conversation History')
    expect(out).toContain(dir)
    expect(out).toContain('mcp__user-tools__Read')
  })

  it('omits the history section when no mirror dir is provided', () => {
    expect(buildSystemPrompt(CWD, 'x')).not.toContain('## Conversation History')
    expect(buildSystemPrompt(CWD, 'x', '   ')).not.toContain('## Conversation History')
  })

  it('keeps the user instructions LAST, after the history section', () => {
    const out = buildSystemPrompt(CWD, 'MY RULE', 'C:/mirror/transcripts')
    expect(out.indexOf('MY RULE')).toBeGreaterThan(out.indexOf('## Conversation History'))
  })
})
