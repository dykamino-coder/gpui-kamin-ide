import { afterEach, describe, expect, it } from 'vitest'
import { rewriteHooksForCli } from './proxy-rewriter'
import { clearSession, listSession } from './registry'
import { matcherAccepts, matcherForCli, payloadForHook } from './tool-names'
import type { HookInputPayload, HookSettings } from './types'

const NATIVE = ['Read', 'Write', 'Edit', 'NotebookEdit', 'Bash', 'PowerShell']
const SESSION = 'native-tool-matchers'
afterEach(() => clearSession(SESSION))

/** Tool names a CLI-bound matcher fires for, out of everything the model can call. */
function firesFor(matcher: string | undefined): string[] {
  const callable = [...NATIVE.map((name) => `mcp__user-tools__${name}`), 'Agent', 'mcp__memory__write_note']
  return callable.filter((name) => matcherAccepts(matcher, name))
}

describe('hook matcher for Bridge tools', () => {
  it('follows the Claude Code rule: exact names in a list, otherwise an unanchored regex', () => {
    expect(matcherAccepts('Edit|Write', 'Write')).toBe(true)
    expect(matcherAccepts('Edit, Write', 'Edit')).toBe(true)
    expect(matcherAccepts('Edit|Write', 'NotebookEdit')).toBe(false)
    expect(matcherAccepts('Edit.*', 'NotebookEdit')).toBe(true)
    expect(matcherAccepts('^Edit$', 'NotebookEdit')).toBe(false)
    expect(matcherAccepts('mcp__user-tools', 'mcp__user-tools__Write')).toBe(false)
    expect(matcherAccepts('(', 'Write')).toBe(false)
    expect(matcherAccepts(undefined, 'Write')).toBe(true)
    expect(matcherAccepts('*', 'Write')).toBe(true)
  })

  it('extends a list of native names with the Bridge tools that replace them', () => {
    expect(matcherForCli('PostToolUse', 'Edit|Write', NATIVE)).toBe(
      'Edit|Write|mcp__user-tools__Write|mcp__user-tools__Edit',
    )
    expect(firesFor(matcherForCli('PostToolUse', 'Edit|Write', NATIVE))).toEqual([
      'mcp__user-tools__Write',
      'mcp__user-tools__Edit',
    ])
    expect(firesFor(matcherForCli('PreToolUse', 'Bash', NATIVE))).toEqual(['mcp__user-tools__Bash'])
    // A name Bridge does not replace stays as written.
    expect(matcherForCli('PreToolUse', 'Agent', NATIVE)).toBe('Agent')
  })

  it('extends a regular expression by exactly the native tools it matches', () => {
    expect(firesFor(matcherForCli('PreToolUse', '^(Edit|Write)$', NATIVE))).toEqual([
      'mcp__user-tools__Write',
      'mcp__user-tools__Edit',
    ])
    expect(firesFor(matcherForCli('PreToolUse', '^Notebook', NATIVE))).toEqual(['mcp__user-tools__NotebookEdit'])
    // `Edit.*` is unanchored and already matches both Bridge edit tools.
    expect(matcherForCli('PreToolUse', 'Edit.*', NATIVE)).toBe('Edit.*')
    expect(firesFor(matcherForCli('PreToolUse', 'mcp__memory__.*', NATIVE))).toEqual(['mcp__memory__write_note'])
  })

  it('leaves alone what already matches, other events and hooks without a matcher', () => {
    expect(matcherForCli('PreToolUse', undefined, NATIVE)).toBeUndefined()
    expect(matcherForCli('PreToolUse', '', NATIVE)).toBe('')
    expect(matcherForCli('PreToolUse', '*', NATIVE)).toBe('*')
    expect(matcherForCli('PreToolUse', 'mcp__user-tools__Write', NATIVE)).toBe('mcp__user-tools__Write')
    expect(matcherForCli('PreToolUse', 'Write|mcp__user-tools__Write', NATIVE)).toBe('Write|mcp__user-tools__Write')
    expect(matcherForCli('SessionStart', 'Bash', NATIVE)).toBe('Bash')
    expect(matcherForCli('SubagentStop', 'Bash', NATIVE)).toBe('Bash')
  })

  it('writes the extended matcher to the CLI settings and keeps the user matcher in the registry', () => {
    const hooks: HookSettings = {
      PostToolUse: [
        { matcher: 'Edit|Write', hooks: [{ type: 'command', command: 'node hook.js' }] },
        { hooks: [{ type: 'command', command: 'node all.js' }] },
      ],
      PreToolUse: [{ matcher: 'Bash', hooks: [{ type: 'command', command: 'node bash.js' }] }],
    }
    const rewritten = rewriteHooksForCli(SESSION, hooks, { kind: 'user', client: true })
    expect(rewritten.PostToolUse?.map((m) => m.matcher)).toEqual([
      'Edit|Write|mcp__user-tools__Write|mcp__user-tools__Edit',
      undefined,
    ])
    expect(rewritten.PreToolUse?.[0]?.matcher).toBe('Bash|mcp__user-tools__Bash')
    // One CLI entry per user hook: nothing is registered or fired twice.
    expect(rewritten.PostToolUse?.map((m) => m.hooks.length)).toEqual([1, 1])
    expect(
      listSession(SESSION)
        .map((hook) => hook.matcher ?? '')
        .sort(),
    ).toEqual(['', 'Bash', 'Edit|Write'])
  })
})

describe('tool name in the hook input', () => {
  const input = (tool_name: string): HookInputPayload =>
    ({
      hook_event_name: 'PostToolUse',
      session_id: 's',
      tool_name,
      tool_input: { file_path: 'a.txt' },
    }) as HookInputPayload
  const name = (matcher: string | undefined, tool: string) =>
    (payloadForHook(matcher, input(tool), NATIVE) as { tool_name?: string }).tool_name

  it('is the native name for a hook written for native names', () => {
    expect(name('Edit|Write', 'mcp__user-tools__Write')).toBe('Write')
    expect(name(undefined, 'mcp__user-tools__Bash')).toBe('Bash')
    expect(name('*', 'mcp__user-tools__Bash')).toBe('Bash')
  })

  it('stays the MCP name for a hook that matches only the MCP name, and for other tools', () => {
    expect(name('mcp__user-tools__Write', 'mcp__user-tools__Write')).toBe('mcp__user-tools__Write')
    expect(name('^mcp__user-tools__', 'mcp__user-tools__Write')).toBe('mcp__user-tools__Write')
    expect(name(undefined, 'mcp__memory__write_note')).toBe('mcp__memory__write_note')
    expect(name(undefined, 'mcp__user-tools__ShowWidget')).toBe('mcp__user-tools__ShowWidget')
    expect(name(undefined, 'Agent')).toBe('Agent')
  })

  it('does not change the payload object it was given', () => {
    const original = input('mcp__user-tools__Write')
    payloadForHook(undefined, original, NATIVE)
    expect((original as { tool_name?: string }).tool_name).toBe('mcp__user-tools__Write')
  })
})
