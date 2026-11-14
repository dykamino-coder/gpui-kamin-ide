import { describe, expect, it } from 'vitest'
import { powershellRuleMatches } from './rule-matchers'

// Native Windows CLI 2.1.322 PreToolUse results from an isolated local-API
// fixture. Unquoted simple variables and the following '=' are separate AST
// elements. Quotes, braced variables and literal prefixes keep one argument.
const rules = [
  'Write-Output $x =fixture',
  'Write-Output $x=fixture',
  'Write-Output $x =y=z',
  'Write-Output $env:NAME =fixture',
  'Write-Output $env:NAME=fixture',
  'Write-Output ${x}=fixture',
  'Write-Output abc$x=fixture',
  'Write-Output name=value',
  'Get-Process *',
  '*',
]

describe.each([
  ['Write-Output $x=fixture', 'Write-Output $x =fixture'],
  ['Write-Output $x=y=z', 'Write-Output $x =y=z'],
  ['Write-Output $env:NAME=fixture', 'Write-Output $env:NAME =fixture'],
  ["Write-Output '$x=fixture'", 'Write-Output $x=fixture'],
  ['Write-Output "$x=fixture"', 'Write-Output $x=fixture'],
  ['Write-Output ${x}=fixture', 'Write-Output ${x}=fixture'],
  ['Write-Output abc$x=fixture', 'Write-Output abc$x=fixture'],
  ['Write-Output name=value', 'Write-Output name=value'],
])('PowerShell variable condition: %s', (command, nativeRule) => {
  it.each(rules)('compares %s with native AST element boundaries', (rule) => {
    expect(powershellRuleMatches(rule, command!)).toBe(rule === nativeRule || rule === '*')
  })
})
