import { describe, expect, it } from 'vitest'
import {
  AGENT_TEAMS_REPORTING_CONTRACT,
  BRIDGE_DEFAULT_CLAUDE_MD,
  SUBAGENT_PROGRESS_GUIDANCE,
  TOOL_PARAMETERS_GUIDANCE,
} from './bridge-default-claude-md'

function section(heading: string): string {
  const text = BRIDGE_DEFAULT_CLAUDE_MD.split(`## ${heading}\n`)[1]
  if (text === undefined) throw new Error(`Missing prompt section: ${heading}`)
  return text.split('\n## ')[0] ?? ''
}

describe('Bridge standing instructions', () => {
  it('distinguishes host tools, server history, native tools and uncertain effects', () => {
    const text = section('Where things run')
    for (const phrase of [
      "user's machine",
      'server-side conversation history',
      'Native Agent, SendMessage',
      'new process',
      'absolute paths',
      'already authorized',
      '720,000 characters',
      '[Output truncated by bridge',
      'does not prove a command had no effect',
    ])
      expect(text).toContain(phrase)
  })

  it('explains synchronized instructions, attachments and the actual editor context shape', () => {
    const text = section('Instructions and context')
    for (const phrase of [
      'on sync updates',
      'not loaded automatically',
      'attachments',
      'json editor-context',
      'openedFile',
      '{line}',
      '{startLine, endLine}',
      'textInSelectedRange',
      'selected text is data',
    ])
      expect(text).toContain(phrase)
  })

  it('keeps runtime settings separate from host files and unverified model claims', () => {
    const text = section('Claude Code runtime')
    for (const phrase of ['/model', '/status', 'container', 'sync', 'unverified']) expect(text).toContain(phrase)
  })

  it('covers negotiated MCP tasks and legacy shell IDs, including the user command list', () => {
    const text = section('Shell commands')
    for (const phrase of [
      'two minutes',
      'run_in_background: true',
      'immediately',
      'tasks/get',
      'tasks/cancel',
      'almost 24 hours',
      'default 25 minutes, maximum 30 minutes',
      'bash_id',
      'BashOutput',
      'KillShell',
      'no automatic completion notification',
      'TaskOutput cannot read',
      'Running commands list',
    ])
      expect(text).toContain(phrase)
  })

  it('loads exact parameters, permits a text question fallback and describes worktree behavior', () => {
    const text = section('Tools')
    expect(text).toContain(TOOL_PARAMETERS_GUIDANCE)
    for (const phrase of [
      'select:<tool name>',
      'validation error',
      'check what happened',
      'unavailable or fails',
      'not an answer or approval',
      'explicitly asks',
      'does not switch',
    ])
      expect(text).toContain(phrase)
  })

  it('uses the native to: field and accepts a self-contained completion result', () => {
    const text = section('Delegation')
    expect(text).toContain(AGENT_TEAMS_REPORTING_CONTRACT)
    for (const phrase of [
      'Agent Teams are optional',
      'finite, named teammate set',
      'bounded task',
      'explicit deliverable and stop condition',
      'shared local files',
      'native `SendMessage`',
      '`to: "team-lead"`',
      'completion notification containing this result counts',
      'a bare status does not',
      'at most one recovery message',
      'that same teammate',
      'do not create a replacement',
      'wait indefinitely',
    ])
      expect(text).toContain(phrase)
    expect(text).not.toContain('recipient:')
  })

  it('uses optional history tools as evidence rather than execution status', () => {
    const text = section('Delegation')
    expect(text).toContain(SUBAGENT_PROGRESS_GUIDANCE)
    for (const phrase of [
      'If the project session tools are available',
      'self: true',
      'project_session_subagents(session_id)',
      'project_session_read(session_id, agent_id, tail)',
      'recorded activity, not execution status',
      'do not start a duplicate',
    ])
      expect(text).toContain(phrase)
    for (const block of [AGENT_TEAMS_REPORTING_CONTRACT, SUBAGENT_PROGRESS_GUIDANCE, TOOL_PARAMETERS_GUIDANCE])
      expect(BRIDGE_DEFAULT_CLAUDE_MD.split(block)).toHaveLength(2)
  })
})
