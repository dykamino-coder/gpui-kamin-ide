// Regression cases sampled against the native CLI 2.1.322 PowerShell matcher.
import { describe, expect, it } from 'vitest'
import { powershellRuleMatches } from './rule-matchers'

describe('PowerShell hook conditions with nested expressions', () => {
  it.each([
    ['Write-Output "prefix$(Get-Location)"', 'Write-Output prefix$(Get-Location)'],
    ['Write-Output { Get-Location }', 'Write-Output { Get-Location }'],
  ])('traverses command AST children of %s without accepting unrelated rules', (command, outer) => {
    expect(powershellRuleMatches('Get-Location', command)).toBe(true)
    expect(powershellRuleMatches(outer, command)).toBe(true)
    for (const rule of ['Get-Process *', 'Out-String', 'Write-Output fixture', 'echo *']) {
      expect(powershellRuleMatches(rule, command)).toBe(false)
    }
  })

  it.each([
    ['Get-Location', '(Get-Location)', true],
    ['Get-Location', 'Write-Output $(Get-Location)', true],
    ['Write-Output *', 'Write-Output $(Get-Location)', true],
    ['Write-Output *', "Write-Output @('a','b')", true],
    ['Get-Location', 'Write-Output (Get-Location)', true],
    ['Out-String', '(Get-Location) | Out-String', true],
    ['Get-Location', "Write-Output @('a(b)','c)d')", false],
    ['Get-Location', 'Write-Output $((Get-Location))', true],
    ['Get-Location', 'Write-Output @(Get-Location)', true],
    ['Get-Location', "Write-Output @( 'a', 'b' )", false],
    ['Out-String', 'Write-Output $(Get-Location | Out-String)', true],
    ['Get-Location', "Write-Output $('a','b')", false],
    ['Get-Process *', 'Write-Output $(Get-Location; Get-Location)', false],
    ["Write-Output a'b", "Write-Output @('a''b','c')", false],
  ] as const)('%s accepts %s: %s', (rule, command, expected) => {
    expect(powershellRuleMatches(rule, command)).toBe(expected)
  })

  it.each([
    'Get-Location',
    'Get-Process *',
    'Write-Output fixture',
    'Write-Output a b',
    'Out-String',
    'echo *',
    'Write-Output prefix*',
    'Write-Output $env:HOME',
    'Write-Output $env:UNSET',
    "Write-Output a'b",
    'Write-Output fi*',
  ])('%s rejects a literal array argument', (rule) => {
    expect(powershellRuleMatches(rule, "Write-Output @('a','b')")).toBe(false)
  })

  it.each([
    'Get-Process *',
    'Write-Output fixture',
    'Write-Output a b',
    'Out-String',
    'echo *',
    'Write-Output prefix*',
    'Write-Output $env:HOME',
    'Write-Output $env:UNSET',
    "Write-Output a'b",
    'Write-Output fi*',
  ])('%s rejects an unrelated outer and nested command', (rule) => {
    expect(powershellRuleMatches(rule, 'Write-Output $(Get-Location)')).toBe(false)
  })
})
