// Expected results sampled from native Windows CLI 2.1.322.
import { describe, expect, it } from 'vitest'
import { powershellRuleMatches } from './rule-matchers'

describe('PowerShell hook conditions around literal tokens', () => {
  it.each([
    ['Get-Location', 'Write-Output --% $(Get-Location)', false],
    ['Write-Output --% $(Get-Location)', 'Write-Output --% $(Get-Location)', true],
    ['Write-Output fixture', 'Write-Output <# block comment #> fixture', true],
    ['Write-Output fixture', 'Write-Output <# outer <# nested #> outer #> fixture', false],
    ['Get-Location', 'Write-Output $(<# comment ) { #> Get-Location)', true],
    ['Get-Location', '& "C:\\fixture\\Get-Location"', false],
    ['C:\\fixture\\Get-Location', '& "C:\\fixture\\Get-Location"', true],
    ['Get-Location', '& ".\\Get-Location"', false],
    ['Get-Location', '& "./Get-Location"', false],
    ['Get-Location', 'Microsoft.PowerShell.Management\\Get-Location', true],
    ['Microsoft.PowerShell.Management\\Get-Location', 'Microsoft.PowerShell.Management\\Get-Location', false],
  ] as const)('%s against %j => %s', (rule, command, expected) => {
    expect(powershellRuleMatches(rule, command)).toBe(expected)
  })
})
