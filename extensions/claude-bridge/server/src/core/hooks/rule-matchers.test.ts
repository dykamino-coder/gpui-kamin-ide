import { describe, expect, it } from 'vitest'
import { bashRuleMatches } from './rule-matchers'

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
