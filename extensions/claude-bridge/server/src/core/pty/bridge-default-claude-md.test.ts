import { describe, expect, it } from 'vitest'

import {
  AGENT_TEAMS_REPORTING_CONTRACT,
  BRIDGE_DEFAULT_CLAUDE_MD,
  SUBAGENT_PROGRESS_GUIDANCE,
  TOOL_PARAMETERS_GUIDANCE,
} from './bridge-default-claude-md'

describe('Agent Teams reporting contract', () => {
  it('is conditional and does not force a team for every task', () => {
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('If you choose to use Agent Teams')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('Agent Teams are optional')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).not.toMatch(/always use Agent Teams|for every task/i)
  })

  it('requires bounded assignments and a finite expected teammate set', () => {
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('finite, named teammate set')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('bounded independent task')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('explicit deliverable and stop condition')
  })

  it('requires delivery through SendMessage before idle', () => {
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('native `SendMessage` tool')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('`recipient: "team-lead"`')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('before becoming idle')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('status, findings, artifact paths, and blockers')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('not the report itself')
  })

  it('permits one bounded recovery without replacement or an infinite loop', () => {
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('at most one recovery `SendMessage`')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('that same teammate')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('Do not create a replacement teammate')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('retry in a loop, or wait indefinitely')
    expect(AGENT_TEAMS_REPORTING_CONTRACT).toContain('synthesize only the reports that were actually delivered')
  })

  it('installs the reporting contract in the shared Bridge default', () => {
    expect(BRIDGE_DEFAULT_CLAUDE_MD.split('## Agent Teams delivery contract')).toHaveLength(2)
  })
})

describe('subagent progress guidance', () => {
  it('points at the project session tools and is part of the default prompt', () => {
    expect(SUBAGENT_PROGRESS_GUIDANCE).toContain('project_session_subagents')
    expect(SUBAGENT_PROGRESS_GUIDANCE).toContain('project_session_read')
    expect(SUBAGENT_PROGRESS_GUIDANCE).toContain('do not start a duplicate')
    expect(BRIDGE_DEFAULT_CLAUDE_MD).toContain(SUBAGENT_PROGRESS_GUIDANCE)
  })
})

describe('tool parameters guidance', () => {
  it('tells agents to load a tool definition instead of guessing and is part of the default prompt', () => {
    expect(TOOL_PARAMETERS_GUIDANCE).toContain('select:<tool name>')
    expect(TOOL_PARAMETERS_GUIDANCE).toContain('instead of guessing argument names')
    expect(BRIDGE_DEFAULT_CLAUDE_MD).toContain(TOOL_PARAMETERS_GUIDANCE)
  })
})
