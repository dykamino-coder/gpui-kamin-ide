// Expected results sampled from native Windows CLI 2.1.322.
import { describe, expect, it } from 'vitest'
import { powershellRuleMatches } from './rule-matchers'

describe('PowerShell hook conditions in hashtable values', () => {
  it.each([
    ['Get-Location', '$ht = @{key=(Get-Location)}', true],
    ['Get-Process *', '$ht = @{key=(Get-Location)}', false],
    ['Get-Location', '$ht=@{key=Get-Location}', true],
    ['Write-Output *', '$ht=@{key=Get-Location}', false],
    ['Get-Location', '@{key=Get-Location; other=Write-Output fixture}', true],
    ['Write-Output fixture', '@{key=Get-Location; other=Write-Output fixture}', true],
    ['key *', '@{key=Get-Location; other=Write-Output fixture}', false],
    ['other *', '@{key=Get-Location; other=Write-Output fixture}', false],
    ['Get-Location', '@{key=1; other="fixture"}', false],
    ['Get-Location', '@{key="$(Get-Location)"}', true],
    ['Write-Output *', '@{key="$(Get-Location)"}', false],
    ['Get-Location', '@{key=@{nested=(Get-Location)}}', true],
    ['Get-Process *', '@{key=@{nested=(Get-Location)}}', false],
    ['Write-Output fixture', '@{key=@(Get-Location; Write-Output fixture)}', true],
    ['Get-Process *', '@{key=@(Get-Location; Write-Output fixture)}', false],
    ['Get-Location', '@{key={Get-Location}}', true],
    ['Write-Output *', '@{key={Get-Location}}', false],
    ['Write-Output @{key=(Get-Location)}', 'Write-Output @{key=(Get-Location)}', true],
    ['Write-Output fixture', 'Write-Output @{key=(Get-Location)}', false],
    ['Get-Location', 'Write-Output @{key=1}', false],
    ['Write-Output @{key=1}', 'Write-Output @{key=1}', true],
    ['Out-Null', '@{key=(Get-Location); other=1} | Out-Null', true],
    ['Get-Process *', '@{key=(Get-Location); other=1} | Out-Null', false],
    ['Get-Location', '@{"key;="=(Get-Location)}', true],
    ['Get-Process *', '@{"key;="=(Get-Location)}', false],
    ['Get-Location', '@{key = <# ;= } #> (Get-Location)}', true],
    ['Get-Process *', '@{key = <# ;= } #> (Get-Location)}', false],
    ['Write-Output fixture', '@{key=(Get-Location)\nother=Write-Output fixture}', true],
    ['Get-Process *', '@{key=(Get-Location)\nother=Write-Output fixture}', false],
    ['Get-Location', '@{key=$null}', false],
    ['Get-Location', '@{key=1+2}', false],
    ['Get-Location', '$ht = @{key=1}; Get-Location', true],
    ['Write-Output *', '$ht = @{key=1}; Get-Location', false],
    ...['@{}', '@()', '(1)', '# comment', ' '].flatMap(
      (command) =>
        [
          ['Get-Location', command, false],
          ['*', command, true],
        ] as const,
    ),
  ] as const)('%s against %j => %s', (rule, command, expected) => {
    expect(powershellRuleMatches(rule, command)).toBe(expected)
  })
})
