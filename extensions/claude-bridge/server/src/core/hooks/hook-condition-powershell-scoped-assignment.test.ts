import { describe, expect, it } from 'vitest'
import { powershellRuleMatches } from './rule-matchers'

// Native Windows CLI 2.1.322 results from an isolated local-API fixture.
// Assignment operators and scoped/braced variables are outside the RHS
// CommandAst, just like an ordinary variable assignment.
describe('PowerShell scoped and compound assignment conditions', () => {
  it.each([
    '$global:x=Write-Output fixture',
    '$script:x = Write-Output fixture',
    '${x}=Write-Output fixture',
    '$x += Write-Output fixture',
    '$x ??= Write-Output fixture',
    '$env:NAME=Write-Output fixture',
    '$x+=Write-Output fixture',
    '$x-=Write-Output fixture',
    '$x*=Write-Output fixture',
    '$x/=Write-Output fixture',
    '$x%=Write-Output fixture',
    '$x??=Write-Output fixture',
    '${global:x} = Write-Output fixture',
    '$local:x= & Write-Output fixture',
    '${env:NAME}=Write-Output fixture',
    '$x = $y = Write-Output fixture',
  ])('matches only the right-hand command in %s', (command) => {
    expect(powershellRuleMatches('Write-Output fixture', command)).toBe(true)
    expect(powershellRuleMatches('Write-Output *', command)).toBe(true)
    expect(powershellRuleMatches('Get-Location', command)).toBe(false)
    expect(powershellRuleMatches('Get-Process *', command)).toBe(false)
    expect(powershellRuleMatches('*', command)).toBe(true)
  })

  it.each([
    '$private:x = (Get-Location)',
    '$global:x += (Get-Location)',
    'Write-Output fixture; $script:x=Get-Location',
  ])('finds nested or subsequent commands in %s', (command) => {
    expect(powershellRuleMatches('Get-Location', command)).toBe(true)
    expect(powershellRuleMatches('Get-Process *', command)).toBe(false)
  })
})
