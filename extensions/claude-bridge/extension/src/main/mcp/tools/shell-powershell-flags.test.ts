import { describe, expect, it } from 'vitest'
import { powerShellFlags } from './shell'

describe('PowerShell flags (native Claude Code parity)', () => {
  it('skips the profile and bypasses the execution policy by default', () => {
    expect(powerShellFlags({})).toEqual(['-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command'])
  })
  it('respects the execution policy when the user opts in', () => {
    expect(powerShellFlags({ CLAUDE_CODE_POWERSHELL_RESPECT_EXECUTION_POLICY: '1' })).toEqual([
      '-NoLogo',
      '-NoProfile',
      '-NonInteractive',
      '-Command',
    ])
    expect(powerShellFlags({ CLAUDE_CODE_POWERSHELL_RESPECT_EXECUTION_POLICY: 'false' })).toContain('Bypass')
  })
})