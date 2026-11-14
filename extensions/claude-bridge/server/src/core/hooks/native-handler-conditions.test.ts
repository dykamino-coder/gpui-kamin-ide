import { describe, expect, it, vi } from 'vitest'
import { register } from '../../cli-mod/hooks/bridge.js'

type Handler = ($: any, e: any, next: any) => any
function moduleHandlers() {
  const handlers = new Map<string, Handler>()
  register((name: string, handler: Handler) => handlers.set(name, handler))
  expect(handlers.has('settings.read')).toBe(true)
  return handlers
}
const settingsHook = () => moduleHandlers().get('settings.read')!

const stateKey = 'CLAUDE_BRIDGE_HOOK_CONDITIONS'
const sessionKey = 'CLAUDE_BRIDGE_HOOK_SESSION'
const declaration = (type: string, condition?: string) => ({
  type,
  ...(condition === undefined ? {} : { if: condition }),
  model: 'fixture-model',
  prompt: 'original $ARGUMENTS',
  server: 'fixture',
  tool: 'Check',
  input: { label: 'original' },
  timeout: 40,
  once: true,
})

async function activeModule() {
  const handlers = moduleHandlers()
  const layers = (marker: any) => [
    { source: 'local', settings: { env: { KEEP: 'original', [stateKey]: JSON.stringify({ mine: marker }) } } },
    { source: 'flag', settings: { env: { [sessionKey]: 'mine' } } },
  ]
  const acknowledge = async (marker: any) => {
    const next = Object.assign(async () => ({ value: {} }), { origin: { plugin: 'engine' } })
    await handlers.get('settings.read')!({}, { layers: layers(marker) }, next)
  }
  await acknowledge({ generation: 'initial' })
  const calls: any[] = []
  const $ = {
    env: {
      get: async (key: string) =>
        ({
          BRIDGE_CLI_MOD_SOCKET: '/fixture.sock',
          BRIDGE_CLI_MOD_SESSION: 'mine',
          BRIDGE_CLI_MOD_HOOK_CONDITIONS: '1',
        })[key],
    },
    session: { version: async () => ({ version: '2.1.322' }) },
    clock: { now: async () => 1, every: vi.fn() },
    fs: { read: vi.fn() },
    http: {
      fetch: vi.fn(async (url: string, options: any) => {
        const body = JSON.parse(options.body)
        calls.push({ route: new URL(url).pathname, body })
        let result = {}
        if (url.endsWith('/prepare') || url.endsWith('/restore')) {
          const generation = body.generation + (url.endsWith('/restore') ? ':restore' : '')
          await acknowledge({ generation })
          result = { generation }
        }
        return { ok: true, text: JSON.stringify(result) }
      }),
    },
  }
  await handlers.get('session.start')!($, { isInteractive: true, surface: 'terminal' }, async () => ({}))
  calls.length = 0
  return { $, handlers, calls }
}

describe('original handler event lifetime', () => {
  it.each([false, true])(
    'continues the original MCP envelope exactly once and restores it after failure=%s',
    async (failure) => {
      const { $, handlers, calls } = await activeModule()
      const input = { tool: 'mcp__user-tools__Bash', tool_use_id: 'call', command: 'echo original' }
      const result = { deny: 'original block' }
      const next = vi.fn(async (e) => {
        expect(e).toBe(input)
        expect(calls.map((c) => c.route)).toEqual(['/hooks/prepare'])
        if (failure) throw new Error('original failure')
        return result
      })
      const promise = handlers.get('classic.PreToolUse')!($, input, next)
      if (failure) await expect(promise).rejects.toThrow('original failure')
      else expect(await promise).toBe(result)
      expect(next).toHaveBeenCalledOnce()
      expect(calls.map((c) => c.route)).toEqual(['/hooks/prepare', '/hooks/restore', '/hooks/release'])
      expect(calls[0].body.payload).toEqual({ tool_name: input.tool, tool_input: { command: input.command } })
    },
  )

  it("also serializes unrelated tools so they cannot see a parallel event's removed predicates", async () => {
    const { $, handlers, calls } = await activeModule()
    await handlers.get('classic.PreToolUse')!($, { tool: 'mcp__other__Check', tool_use_id: 'other' }, async () => ({}))
    expect(calls[0].body.payload.tool_name).toBe('mcp__other__Check')
  })

  it('lets a native hook agent use MCP tools without waiting behind its own parent', async () => {
    const { $, handlers, calls } = await activeModule()
    await handlers.get('tool.call')!($, { tool_use_id: 'nested', agentId: 'hook-agent-fixture' }, async () => {
      await handlers.get('classic.PreToolUse')!(
        $,
        { tool: 'mcp__user-tools__Bash', tool_use_id: 'nested' },
        async () => ({}),
      )
      return {}
    })
    expect(calls).toEqual([])
    await handlers.get('classic.PostToolUse')!(
      $,
      { tool_name: 'mcp__user-tools__Bash', agent_id: 'hook-agent-fixture' },
      async () => ({}),
    )
    expect(calls).toEqual([])
  })

  it.each([0, 1])('retains overlapping hook-agent calls sharing an ID when call %s finishes first', async (first) => {
    const { $, handlers, calls } = await activeModule()
    const releases: (() => void)[] = []
    const running = [0, 1].map((index) =>
      handlers.get('tool.call')!(
        $,
        { tool_use_id: 'shared', agentId: `hook-agent-${index}` },
        () => new Promise<void>((resolve) => releases.push(resolve)),
      ),
    )
    releases[first]!()
    await running[first]
    await handlers.get('classic.PreToolUse')!(
      $,
      { tool: 'mcp__user-tools__Bash', tool_use_id: 'shared' },
      async () => ({}),
    )
    expect(calls).toEqual([])
    releases[1 - first]!()
    await running[1 - first]
    await handlers.get('classic.PreToolUse')!(
      $,
      { tool: 'mcp__user-tools__Bash', tool_use_id: 'shared' },
      async () => ({}),
    )
    expect(calls.map((call) => call.route)).toEqual(['/hooks/prepare', '/hooks/restore', '/hooks/release'])
  })

  it('suppresses only marker-only ConfigChange events and preserves real edits and other sources', async () => {
    const { $, handlers } = await activeModule()
    const event = { source: 'local_settings', file_path: '/fixture/settings.local.json' }
    const next = vi.fn(async () => ({ original: true }))
    $.fs.read.mockResolvedValue(JSON.stringify({ env: { KEEP: 'original', [stateKey]: 'changed' } }))
    expect(await handlers.get('classic.ConfigChange')!($, event, next)).toEqual({})
    expect(next).not.toHaveBeenCalled()
    $.fs.read.mockResolvedValue(JSON.stringify({ env: { KEEP: 'user edit', [stateKey]: 'another marker' } }))
    expect(await handlers.get('classic.ConfigChange')!($, event, next)).toEqual({ original: true })
    expect(next).toHaveBeenCalledOnce()
    await handlers.get('classic.ConfigChange')!($, { ...event, source: 'user_settings' }, next)
    expect(next).toHaveBeenCalledTimes(2)
    $.fs.read.mockRejectedValue(new Error('file removed'))
    await handlers.get('classic.ConfigChange')!($, event, next)
    expect(next).toHaveBeenCalledTimes(3)
  })
})

async function read(hooks: any, decisions: any, session = 'mine', source?: string) {
  const original = { hooks }
  const layers = [
    { source: 'local', settings: { env: { [stateKey]: JSON.stringify(decisions) } } },
    { source: 'flag', settings: { env: { [sessionKey]: session } } },
  ]
  const result = await settingsHook()({}, { layers, source }, async () => ({ value: original }))
  return { original, result: result.value }
}

describe('original CLI handler argument conditions', () => {
  it.each(['prompt', 'agent', 'mcp_tool'])('filters %s without replacing the original handler', async (type) => {
    const accepted = declaration(type, 'Bash(echo *)')
    const rejected = declaration(type, 'Bash(git *)')
    const unchanged = [declaration(type), declaration(type, 'mcp__user-tools__Bash')]
    const command = declaration('command', 'Bash(echo *)')
    const http = declaration('http', 'Bash(echo *)')
    const hooks = {
      PreToolUse: [{ matcher: 'Bash|mcp__user-tools__Bash', hooks: [accepted, rejected, ...unchanged, command, http] }],
      PostToolUse: [{ matcher: '.*', hooks: [accepted, rejected] }],
    }
    const { original, result } = await read(hooks, {
      mine: { generation: '1', event: 'PreToolUse', rules: { 'Bash(echo *)': true, 'Bash(git *)': false } },
    })
    const { if: _condition, ...withoutCondition } = accepted
    expect(result.hooks.PreToolUse[0]).toEqual({
      matcher: 'Bash|mcp__user-tools__Bash',
      hooks: [withoutCondition, ...unchanged, command, http],
    })
    expect(result.hooks.PostToolUse).toEqual(hooks.PostToolUse)
    expect(original.hooks).toEqual(hooks)
    expect(original.hooks.PreToolUse[0].hooks).toContain(accepted)
    expect(accepted.if).toBe('Bash(echo *)')
  })

  it('uses only the session selected by the immutable flag layer', async () => {
    const hooks = { PreToolUse: [{ hooks: [declaration('prompt', 'Read(./notes/**)')] }] }
    const decisions = {
      mine: { generation: '1', event: 'PreToolUse', rules: { 'Read(./notes/**)': true } },
      other: { generation: '8', event: 'PreToolUse', rules: { 'Read(./notes/**)': false } },
    }
    expect((await read(hooks, decisions)).result.hooks.PreToolUse[0].hooks).toHaveLength(1)
    expect((await read(hooks, decisions, 'other')).result.hooks.PreToolUse[0].hooks).toHaveLength(0)
    expect((await read(hooks, decisions, 'missing')).result.hooks).toEqual(hooks)
  })

  it('restores the declarations and leaves source-specific reads intact', async () => {
    const hooks = { PreToolUse: [{ hooks: [declaration('agent', 'Bash(echo *)')] }] }
    expect((await read(hooks, { mine: { generation: '2' } })).result.hooks).toEqual(hooks)
    const decisions = { mine: { generation: '1', event: 'PreToolUse', rules: { 'Bash(echo *)': false } } }
    expect((await read(hooks, decisions, 'mine', 'user')).result.hooks).toEqual(hooks)
    // Re-reading the same layers must give the same result, independent of
    // the event gate's mutable acknowledgement state.
    expect((await read(hooks, decisions)).result).toEqual((await read(hooks, decisions)).result)
  })
})
