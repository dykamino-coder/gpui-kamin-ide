import { describe, expect, it } from 'vitest'
import { powershellRuleMatches } from './rule-matchers'

// Sampled against native Windows CLI 2.1.322 with an isolated local API.
describe('PowerShell conditions with typed and member assignment targets', () => {
  it.each([
    '[string]$x = Write-Output fixture',
    '[string] $x=Write-Output fixture',
    '[System.String]$x=Write-Output fixture',
    '[string[]]$x = Write-Output fixture',
    '[ValidateNotNullOrEmpty()][string]$x=Write-Output fixture',
    '[string]${global:x} = Write-Output fixture',
    '$x[0] = Write-Output fixture',
    '$x.Name=Write-Output fixture',
    '$x,$y=Write-Output fixture',
    '$x.foo += Write-Output fixture',
    "$x['name']=Write-Output fixture",
    '$x[0][1].Name=Write-Output fixture',
    '[string][object]$x=Write-Output fixture',
  ])('extracts the right-hand command in %s', (command) => {
    expect(powershellRuleMatches('Write-Output fixture', command)).toBe(true)
    expect(powershellRuleMatches('Write-Output *', command)).toBe(true)
    expect(powershellRuleMatches('Get-Location', command)).toBe(false)
    expect(powershellRuleMatches('Get-Process *', command)).toBe(false)
  })

  it.each(['[int]$x = Get-Location', '[string]$x=(Get-Location)'])(
    'retains the right-hand command without unrelated candidates in %s',
    (command) => {
      expect(powershellRuleMatches('Get-Location', command)).toBe(true)
      expect(powershellRuleMatches('Write-Output *', command)).toBe(false)
      expect(powershellRuleMatches('Get-Process *', command)).toBe(false)
    },
  )

  it('also traverses the command inside an assignment index', () => {
    const command = '$x[(Get-Location)]=Write-Output fixture'
    expect(powershellRuleMatches('Get-Location', command)).toBe(true)
    expect(powershellRuleMatches('Write-Output fixture', command)).toBe(true)
    expect(powershellRuleMatches('Get-Process *', command)).toBe(false)
  })

  it('also traverses a script block in a validation attribute', () => {
    const command = '[ValidateScript({Get-Location})]$x=Write-Output fixture'
    expect(powershellRuleMatches('Get-Location', command)).toBe(true)
    expect(powershellRuleMatches('Write-Output fixture', command)).toBe(true)
    expect(powershellRuleMatches('Get-Process *', command)).toBe(false)
  })

  it.each(["$x = 'literal'", "[string]$x='literal'", '[string]$x=1+2', '$x=1+2', '$x=$y', '[string]$x=$y'])(
    'does not turn a scalar RHS into a command in %s',
    (command) => {
      for (const rule of ['literal', '1', '1+2', '$y', 'Write-Output *', 'Get-Location']) {
        expect(powershellRuleMatches(rule, command)).toBe(false)
      }
      expect(powershellRuleMatches('*', command)).toBe(true)
    },
  )

  it('retains equals inside a right-hand command argument', () => {
    const command = '[string]$x=Write-Output name=value'
    expect(powershellRuleMatches('Write-Output name=value', command)).toBe(true)
    expect(powershellRuleMatches('Write-Output name', command)).toBe(false)
  })

  it('retains a type-like command argument', () => {
    const command = 'Write-Output [string]$x=fixture'
    expect(powershellRuleMatches(command, command)).toBe(true)
    expect(powershellRuleMatches('Write-Output fixture', command)).toBe(false)
  })

  it.each(['$x.Name', '$x[0]'])('keeps an unquoted %s argument separate from adjacent equals', (target) => {
    const command = `Write-Output ${target}=fixture`
    expect(powershellRuleMatches(`Write-Output ${target} =fixture`, command)).toBe(true)
    expect(powershellRuleMatches(command, command)).toBe(false)
    expect(powershellRuleMatches(command, `Write-Output '${target}=fixture'`)).toBe(true)
  })
})
