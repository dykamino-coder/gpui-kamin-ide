import { describe, expect, it } from 'vitest'
import { bashRuleMatches, powershellRuleMatches } from './rule-matchers'

describe('Bash hook conditions across line continuations', () => {
  // Real CLI 2.1.322 falls back to accepting the rule for a continuation
  // inside a word. A standalone continuation does not add an empty argument.
  it.each([
    ['echo fixture', 'echo "fixtu\\\nre"', true],
    ['echo fixtu re', 'echo "fixtu\\\nre"', true],
    ['echo fixture', 'echo fixtu\\\nre', true],
    ['echo', 'echo \\\n', true],
    ['echo fixture', 'echo \\\nfixture', true],
    ['echo fixture', 'echo "fixture" \\\n', true],
    ['echo fixture', 'echo "fixture\\\n"', true],
    ['echo fixture', "echo 'fixtu\\\nre'", true],
    ['git *', 'echo "fixtu\\\nre"', true],
    ['echo fixture', 'pwd; echo "fixtu\\\nre"', true],
    ['git *', 'echo \\\nfixture', false],
    ['git *', 'echo "fixture" \\\n', false],
  ] as const)('%s against %j => %s', (rule, command, expected) => {
    expect(bashRuleMatches(rule, command)).toBe(expected)
  })
})

describe('PowerShell hook conditions around command syntax', () => {
  // Expected results come from native Windows CLI 2.1.322 PreToolUse hooks,
  // except CRLF, checked directly with the Windows PowerShell AST parser.
  it.each([
    ['Write-Output fixture', 'Write-Output fixture # comment', true],
    ['Write-Output fixture', '# fixture\nWrite-Output fixture', true],
    ['Write-Output fixture', '& Write-Output fixture', true],
    ['Write-Output fixture', '. Write-Output fixture', true],
    ['Write-Output fixture', '$x = Write-Output fixture', true],
    ['Write-Output fixture', 'Write-Output fixture > $null', true],
    ['Write-Output fixture', 'Write-Output fixture 2>$null', true],
    ['Write-Output fixture', 'Write-Output `\n fixture', true],
    ['Write-Output fixture', 'Write-Output fixture`\n', false],
    ['Write-Output fixture', 'Write-Output fixture`\r\n', false],
    ['Get-Location', 'Write-Output fixture # Get-Location', false],
    ['Get-Location', 'Write-Output fixture > Get-Location', false],
    ['Write-Output fixture', "Write-Output 'fixture # comment'", false],
    ['Write-Output fixture', "Write-Output 'fixture > $null'", false],
  ] as const)('%s against %j => %s', (rule, command, expected) => {
    expect(powershellRuleMatches(rule, command)).toBe(expected)
  })
})
