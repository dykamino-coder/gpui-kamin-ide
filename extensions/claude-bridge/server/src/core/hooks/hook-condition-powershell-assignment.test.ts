import { describe, expect, it } from 'vitest'
import { powershellRuleMatches } from './rule-matchers'

describe('PowerShell assignment conditions', () => {
  // Sampled native Windows CLI 2.1.322 PreToolUse results. Assignment syntax
  // is outside the right-hand command, regardless of whitespace around '='.
  it.each([
    '$x=Write-Output fixture',
    '$x =Write-Output fixture',
    '$x= Write-Output fixture',
    '$x = Write-Output fixture',
    '$x= & Write-Output fixture',
  ])('finds the command in %s', (command) => {
    expect(powershellRuleMatches('Write-Output fixture', command)).toBe(true)
    expect(powershellRuleMatches('Get-Process *', command)).toBe(false)
  })

  it.each([
    '$x=Get-Location',
    '$x =Get-Location',
    '$x= Get-Location',
    '$x = Get-Location',
    '$x=(Get-Location)',
    '$x =(Get-Location)',
    '$x= (Get-Location)',
    '$x = (Get-Location)',
    'Write-Output fixture; $x=Get-Location',
  ])('finds the nested or subsequent command in %s', (command) => {
    expect(powershellRuleMatches('Get-Location', command)).toBe(true)
    expect(powershellRuleMatches('Get-Process *', command)).toBe(false)
  })

  it.each([
    '$x=Write-Output name=value',
    '$x =Write-Output name=value',
    '$x= Write-Output name=value',
    '$x = Write-Output name=value',
  ])('retains equals inside an argument in %s', (command) => {
    expect(powershellRuleMatches('Write-Output name=value', command)).toBe(true)
    expect(powershellRuleMatches('Write-Output name', command)).toBe(false)
  })

  it('retains a quoted assignment-like argument', () => {
    expect(powershellRuleMatches('Write-Output $x=fixture', "Write-Output '$x=fixture'")).toBe(true)
    expect(powershellRuleMatches('Write-Output fixture', "Write-Output '$x=fixture'")).toBe(false)
  })
})
