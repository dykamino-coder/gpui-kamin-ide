import { afterEach, describe, expect, it, vi } from 'vitest'
import { EventEmitter } from 'node:events'

vi.mock('../config', () => ({ config: { port: 3030 } }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn() }))
vi.mock('./audit-log', () => ({ appendExecution: vi.fn() }))
import { appendExecution } from './audit-log'
import { dispatchHook, expandedHttpHeaders, handleLocalHookResponse } from './dispatcher'
import { conditionForCli, conditionHolds, parseRule } from './hook-condition'
import { httpHookAnswer } from './http-hook-answer'
import { rewriteHooksForCli } from './proxy-rewriter'
import { clearSession, listSession } from './registry'
import { bashRuleMatches, powershellRuleMatches, wildcardMatches } from './rule-matchers'
import { pathRuleMatches } from './path-rule'
import type { HookHandler, HookInputPayload } from './types'

const SESSION = 'native-condition'
const payload = (command: string): HookInputPayload => ({
  hook_event_name: 'PreToolUse',
  session_id: SESSION,
  tool_name: 'mcp__user-tools__Bash',
  tool_input: { command },
  cwd: '/workspace',
})
afterEach(() => {
  clearSession(SESSION)
  vi.restoreAllMocks()
  vi.unstubAllEnvs()
  vi.clearAllMocks()
})

describe('native hook condition translation', () => {
  it.each(['Bash', 'Bash()', 'Bash(*)'])('maps bare tool %s', (condition) => {
    expect(conditionForCli('PreToolUse', condition)).toEqual({ cliIf: 'mcp__user-tools__Bash' })
  })
  it('keeps MCP rules, other events, unknown tools and malformed syntax with the CLI', () => {
    for (const condition of ['mcp__other__Bash(git *)', 'Agent(x)', 'Bash(git *)tail']) {
      expect(conditionForCli('PreToolUse', condition)).toEqual({ cliIf: condition })
    }
    expect(conditionForCli('Stop', 'Bash')).toEqual({ cliIf: 'Bash' })
    expect(parseRule(String.raw`Edit(foo\(bar\).ts)`)).toEqual({ tool: 'Edit', content: 'foo(bar).ts' })
  })
  it.each(['local', 'server'] as const)('relays argument conditions for %s commands', (host) => {
    const cli = rewriteHooksForCli(
      SESSION,
      {
        PreToolUse: [
          { matcher: 'Bash', hooks: [{ type: 'command', command: 'fixture', host, if: 'Bash(git *)', once: true }] },
        ],
      },
      { kind: 'user' },
    )
    expect(cli.PreToolUse![0]!.hooks[0]).toMatchObject({ type: 'http', if: 'mcp__user-tools__Bash' })
    expect(cli.PreToolUse![0]!.hooks[0]!.once).toBe(true)
    expect(listSession(SESSION)[0]!.serverCondition).toEqual({ tool: 'Bash', content: 'git *' })
  })
  it('keeps async command semantics and leaves prompt argument conditions pending', () => {
    const cli = rewriteHooksForCli(
      SESSION,
      {
        PreToolUse: [
          {
            hooks: [
              { type: 'command', command: 'fixture', async: true, if: 'Bash(git *)' },
              { type: 'prompt', prompt: 'fixture', if: 'Bash(git *)' },
              { type: 'agent', prompt: 'fixture', if: 'Bash' },
            ],
          },
        ],
      },
      { kind: 'user' },
    )
    expect(cli.PreToolUse![0]!.hooks[0]).toMatchObject({
      type: 'command',
      command: 'node',
      async: true,
      if: 'mcp__user-tools__Bash',
    })
    expect(cli.PreToolUse![0]!.hooks[1]!.if).toBe('Bash(git *)')
    expect(cli.PreToolUse![0]!.hooks[2]!.if).toBe('mcp__user-tools__Bash')
  })
})

describe('native argument rules', () => {
  // Counts and candidates checked against CLI 2.1.322, not shell execution.
  it.each([
    ['git *', 'git --version', true],
    ['git *', "printf '$(pwd)'", false],
    ['git *', "printf 'git status'", false],
    ['git *', 'echo fixture # git status', false],
    ['git *', 'FOO=x git --version >/tmp/out', true],
    ['git:*', 'echo fixture && git --version', true],
    ['git *', 'printf $(pwd)', true],
    ['git *', 'if true; then echo fixture; fi', false],
    ['echo *', 'if true; then echo fixture; fi', true],
    ['echo $HOME', 'echo $HOME', false],
    ['echo *', 'echo fixture | xargs echo', true],
    ['printf $(pwd)', "printf '$(pwd)'", true],
  ] as const)('Bash(%s), %s => %s', (rule, command, expected) => {
    expect(bashRuleMatches(rule, command, '/home/cli')).toBe(expected)
  })
  it('handles literal operators in PowerShell quotes and case-insensitive command names', () => {
    expect(powershellRuleMatches('git *', "Write-Output '$(pwd)'")).toBe(false)
    expect(powershellRuleMatches('git *', "Write-Output 'a|git status'")).toBe(false)
    expect(powershellRuleMatches('Write-Output a|b', "Write-Output 'a|b'")).toBe(true)
    expect(powershellRuleMatches('git:*', 'GIT status')).toBe(true)
  })
  it('matches file patterns relative to the client folder, with gitignore semantics', () => {
    expect(pathRuleMatches('src/**', '/workspace/src/nested/a.ts', { cwd: '/workspace' })).toBe(true)
    expect(pathRuleMatches('src/**', '/other/src/a.ts', { cwd: '/workspace' })).toBe(false)
    expect(pathRuleMatches('*.ts', '/other/a.ts', { cwd: '/workspace' })).toBe(true)
    expect(pathRuleMatches('/src/*.ts', 'C:\\Work\\src\\a.ts', { cwd: 'c:\\work' })).toBe(true)
    expect(pathRuleMatches('src/**', '/workspace/src/../other/a.ts', { cwd: '/workspace' })).toBe(false)
    expect(pathRuleMatches('~/notes/**', '/home/fixture/notes/a.md', { home: '/home/fixture' })).toBe(true)
  })
  it('does not treat a regex or ? as wildcard in Glob/Grep input patterns', () => {
    expect(wildcardMatches('a?b', 'acb')).toBe(false)
    expect(wildcardMatches(String.raw`a\*b`, 'a*b')).toBe(true)
    expect(
      conditionHolds(
        { tool: 'Grep', content: 'test*' },
        { ...payload(''), tool_name: 'mcp__user-tools__Grep', tool_input: { pattern: 'test.*' } },
      ),
    ).toBe(true)
    expect(
      conditionHolds({ tool: 'Bash', content: 'git *' }, { ...payload('git status'), tool_name: 'mcp__other__Bash' }),
    ).toBe(false)
  })
})

it('filters before dispatch/audit and runs each matching settings hook call, including concurrency', async () => {
  rewriteHooksForCli(
    SESSION,
    {
      PreToolUse: [
        { matcher: 'Bash', hooks: [{ type: 'command', command: 'fixture', if: 'Bash(git *)', once: true }] },
      ],
    },
    { kind: 'user', client: true },
  )
  const hook = listSession(SESSION)[0]!
  const inputs: HookInputPayload[] = []
  const ws = Object.assign(new EventEmitter(), {
    readyState: 1,
    send(frame: string) {
      const message = JSON.parse(frame)
      inputs.push(message.payload)
      setImmediate(() =>
        handleLocalHookResponse(message.requestId, {
          stdout: '',
          stderr: '',
          exitCode: 0,
          outcome: 'success',
          durationMs: 0,
        }),
      )
    },
  })
  await dispatchHook(SESSION, hook.id, payload("printf '$(pwd)'"), ws as never)
  expect(inputs).toHaveLength(0)
  expect(appendExecution).not.toHaveBeenCalled()
  await Promise.all([
    dispatchHook(SESSION, hook.id, payload('git status'), ws as never),
    dispatchHook(SESSION, hook.id, payload('git diff'), ws as never),
  ])
  expect(inputs).toHaveLength(2)
  expect(inputs[0]!.tool_name).toBe('Bash')
  expect(appendExecution).toHaveBeenCalledTimes(2)
})

it.each([200, 403])('filters original HTTP hooks and preserves status %s and JSON decision', async (status) => {
  const original: HookHandler = {
    type: 'http',
    url: 'http://fixture/hook',
    if: 'Bash(git *)',
    headers: { 'X-Fixture': 'yes' },
  }
  const cli = rewriteHooksForCli(SESSION, { PreToolUse: [{ hooks: [original] }] }, { kind: 'user' })
  expect(cli.PreToolUse![0]!.hooks[0]).toMatchObject({ type: 'http', if: 'mcp__user-tools__Bash' })
  expect((cli.PreToolUse![0]!.hooks[0] as { url: string }).url).not.toBe(original.url)
  const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response('{"decision":"block"}', { status }))
  const hook = listSession(SESSION)[0]!
  await dispatchHook(SESSION, hook.id, payload('pwd'), null)
  expect(fetchMock).not.toHaveBeenCalled()
  const result = await dispatchHook(SESSION, hook.id, payload('git status'), null)
  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(httpHookAnswer('PreToolUse', result, '')).toEqual({ status, body: '{"decision":"block"}' })
})

it('substitutes only allowed HTTP header variables', () => {
  vi.stubEnv('FIXTURE_ALLOWED', 'value')
  vi.stubEnv('FIXTURE_OTHER', 'omitted')
  expect(expandedHttpHeaders({ A: '$FIXTURE_ALLOWED ${FIXTURE_ALLOWED} $FIXTURE_OTHER' }, ['FIXTURE_ALLOWED'])).toEqual(
    { A: 'value value ' },
  )
})
