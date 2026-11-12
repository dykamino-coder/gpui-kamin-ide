import { describe, expect, it } from 'vitest'
import {
  BRIDGE_INTERACTIVE_TOOL_TIMEOUT_SEC,
  bridgeMcpThreadConfig,
  codexLockdownConfig,
  codexUserConfig,
  mergeTables,
  renderToml,
  renderValue,
  toConfigOverrides,
} from './config-toml'

describe('codex lockdown', () => {
  it('is exactly the verified configuration, rendered as config.toml', () => {
    expect(renderToml(codexUserConfig({ catalogPath: 'C:\\data\\codex\\catalog.json' }))).toBe(
      [
        'cli_auth_credentials_store = "file"',
        'web_search = "live"',
        'model_catalog_json = "C:\\\\data\\\\codex\\\\catalog.json"',
        '',
        '[features]',
        'shell_tool = false',
        'unified_exec = false',
        'view_image = false',
        'goals = false',
        'apps = false',
        'plugins = false',
        'browser_use = false',
        'computer_use = false',
        'image_generation = false',
        'multi_agent = false',
        'memories = false',
        'hooks = false',
        'tool_suggest = false',
        'skill_search = false',
        '',
      ].join('\n'),
    )
  })

  it('renders as -c overrides for the app-server process', () => {
    const args = toConfigOverrides(codexLockdownConfig({ catalogPath: '/data/catalog.json', webSearch: 'disabled' }))
    expect(args.filter((_, i) => i % 2 === 0).every((a) => a === '-c')).toBe(true)
    expect(args.filter((_, i) => i % 2 === 1)).toEqual([
      'web_search="disabled"',
      'model_catalog_json="/data/catalog.json"',
      'features.shell_tool=false',
      'features.unified_exec=false',
      'features.view_image=false',
      'features.goals=false',
      'features.apps=false',
      'features.plugins=false',
      'features.browser_use=false',
      'features.computer_use=false',
      'features.image_generation=false',
      'features.multi_agent=false',
      'features.memories=false',
      'features.hooks=false',
      'features.tool_suggest=false',
      'features.skill_search=false',
    ])
  })
})

describe('bridgeMcpThreadConfig', () => {
  it('points both Bridge servers at the session endpoint with its bearer', () => {
    expect(bridgeMcpThreadConfig({ url: 'http://127.0.0.1:3456/mcp/abc', token: 'tok' })).toEqual({
      mcp_servers: {
        bridge: {
          url: 'http://127.0.0.1:3456/mcp/abc',
          http_headers: { Authorization: 'Bearer tok' },
          omit_tools_from: ['deferred'],
          default_tools_approval_mode: 'approve',
          tool_timeout_sec: 3600,
          required: true,
          disabled_tools: ['AskUserQuestion', 'ExitPlanMode', 'EnterPlanMode'],
        },
        bridge_interactive: {
          url: 'http://127.0.0.1:3456/mcp/abc',
          http_headers: { Authorization: 'Bearer tok' },
          omit_tools_from: ['deferred'],
          default_tools_approval_mode: 'approve',
          tool_timeout_sec: BRIDGE_INTERACTIVE_TOOL_TIMEOUT_SEC,
          required: false,
          enabled_tools: ['AskUserQuestion', 'ExitPlanMode', 'EnterPlanMode'],
        },
      },
    })
    expect(BRIDGE_INTERACTIVE_TOOL_TIMEOUT_SEC).toBe(2_592_000)
  })

  it('honours an explicit timeout and interactive list', () => {
    const cfg = bridgeMcpThreadConfig({ url: 'u', token: 't', toolTimeoutSec: 10, interactiveTools: ['Ask'] }) as any
    expect(cfg.mcp_servers.bridge).toMatchObject({ tool_timeout_sec: 10, disabled_tools: ['Ask'] })
    expect(cfg.mcp_servers.bridge_interactive.enabled_tools).toEqual(['Ask'])
  })
})

describe('TOML rendering', () => {
  it('escapes strings and quotes non-bare keys', () => {
    expect(renderValue('a"b\\c\nd\te\u0001')).toBe('"a\\"b\\\\c\\nd\\te\\u0001"')
    expect(renderValue(['x', 1, true])).toBe('["x", 1, true]')
    expect(renderValue({ Authorization: 'Bearer t', 'x.y': 1 })).toBe('{ Authorization = "Bearer t", "x.y" = 1 }')
    expect(() => renderValue(Number.NaN)).toThrow('finite')
    expect(renderToml({ 'a b': { c: 1 } })).toBe('["a b"]\nc = 1\n')
  })

  it('emits empty tables and nested headers depth-first', () => {
    expect(renderToml({ a: {}, b: { c: { d: 'x' } }, e: 1 })).toBe('e = 1\n\n[a]\n\n[b.c]\nd = "x"\n')
  })

  it('refuses keys that cannot be dotted into -c', () => {
    expect(() => toConfigOverrides({ 'a.b': 1 })).toThrow('cannot be passed as -c')
    expect(toConfigOverrides({ a: { b: undefined, c: [1] } })).toEqual(['-c', 'a.c=[1]'])
  })

  it('merges tables deeply, later scalars win', () => {
    expect(mergeTables({ a: { b: 1, c: 2 }, d: 1 }, { a: { c: 3 }, d: { e: 1 } })).toEqual({
      a: { b: 1, c: 3 },
      d: { e: 1 },
    })
  })
})
