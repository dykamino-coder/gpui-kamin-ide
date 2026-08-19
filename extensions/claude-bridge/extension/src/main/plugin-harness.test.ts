import { afterEach, describe, expect, it } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { collectPluginHooks, hashPluginHook, rewritePluginMcpMatchers } from './sync/plugin-snapshot'
import { buildMonitorTriggerHooks } from './plugin-monitors'
import {
  LSP_MAX_CONTENT_BYTES,
  LSP_MAX_HEADER_BYTES,
  LSP_MAX_STDOUT_BUFFER_BYTES,
  LspFrameDecoder,
  matchLspLanguage,
} from './plugin-lsp'
import {
  assertUniqueEnabledPluginNames,
  mergePluginDeclarations,
  selectPluginNamespaceWinners,
  type InstalledPlugin,
} from './plugin-helpers'

const tempDirs: string[] = []

function tempPlugin(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'kamin-plugin-harness-'))
  tempDirs.push(dir)
  return dir
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) fs.rmSync(dir, { recursive: true, force: true })
})

describe('plugin harness discovery', () => {
  it('merges marketplace and local declarations without dropping same-event hooks', async () => {
    const root = tempPlugin()
    const effective = mergePluginDeclarations(root, 'sample', {
      name: 'catalog-name',
      defaultEnabled: false,
      hooks: { PreToolUse: [{ matcher: 'Read', hooks: [] }] },
      mcpServers: { catalog: { command: 'catalog-server' } },
      userConfig: { token: { type: 'string', sensitive: true } },
      experimental: { monitors: ['catalog-monitor.json'] },
    }, {
      name: 'manifest-name',
      defaultEnabled: true,
      hooks: { PreToolUse: [{ matcher: 'Write', hooks: [] }] },
      mcpServers: { local: { command: 'local-server' } },
      userConfig: { endpoint: { type: 'string' } },
      experimental: { themes: ['./themes'] },
    })

    expect(effective.name).toBe('sample')
    expect(effective.defaultEnabled).toBe(false)
    expect(effective.hooks).toHaveLength(2)
    expect(effective.mcpServers).toEqual({
      catalog: { command: 'catalog-server' },
      local: { command: 'local-server' },
    })
    expect(Object.keys(effective.userConfig)).toEqual(['token', 'endpoint'])
    expect(effective.experimental).toEqual({
      monitors: ['catalog-monitor.json'],
      themes: ['./themes'],
    })

    const hooks = await collectPluginHooks(root, effective)
    expect(hooks.PreToolUse).toHaveLength(2)
  })

  it('merges canonical and manifest-declared hook files', async () => {
    const root = tempPlugin()
    fs.mkdirSync(path.join(root, 'hooks'), { recursive: true })
    fs.writeFileSync(path.join(root, 'hooks', 'hooks.json'), JSON.stringify({
      hooks: { PreToolUse: [{ matcher: 'Read', hooks: [{ type: 'command', command: 'guard-read' }] }] },
    }))
    fs.writeFileSync(path.join(root, 'extra-hooks.json'), JSON.stringify({
      hooks: { PostToolUse: [{ matcher: 'Write', hooks: [{ type: 'command', command: 'audit-write' }] }] },
    }))

    const hooks = await collectPluginHooks(root, { hooks: 'extra-hooks.json' })
    expect(hooks.PreToolUse).toHaveLength(1)
    expect(hooks.PostToolUse).toHaveLength(1)
    expect(hashPluginHook('PreToolUse', 'Read', { type: 'command', command: 'guard-read' }))
      .not.toBe(hashPluginHook('PreToolUse', 'Read', { type: 'command', command: 'changed' }))
  })

  it('turns on-skill monitors into internal Skill hooks', async () => {
    const root = tempPlugin()
    fs.mkdirSync(path.join(root, 'monitors'), { recursive: true })
    fs.writeFileSync(path.join(root, 'monitors', 'monitors.json'), JSON.stringify([
      { name: 'watch', command: 'watch.sh', when: 'on-skill-invoke:debug' },
      { name: 'always', command: 'poll.sh', when: 'always' },
    ]))
    const hooks = await buildMonitorTriggerHooks(root, {}, 'sample@local')
    const handlers = (hooks.PreToolUse?.[0] as any)?.hooks ?? []
    expect(handlers).toHaveLength(1)
    expect(handlers[0].command).toMatch(/^__bridge_monitor_start__:/)
  })

  it('maps plugin MCP matchers and mcp_tool handlers to the scoped bridge server', () => {
    const rewritten = rewritePluginMcpMatchers('sample', {
      PreToolUse: [{
        matcher: 'mcp__plugin_sample_docs__search',
        hooks: [{ type: 'mcp_tool', server: 'plugin:sample:docs', tool: 'audit' }],
      }],
    })
    expect((rewritten.PreToolUse?.[0] as any).matcher).toBe('mcp__user-tools__plugin_sample_docs__search')
    expect((rewritten.PreToolUse?.[0] as any).hooks[0]).toMatchObject({
      type: 'mcp_tool',
      server: 'user-tools',
      tool: 'plugin_sample_docs__audit',
    })
  })

  it('maps native tool matchers to the bridge user-tools server', () => {
    const rewritten = rewritePluginMcpMatchers('sample', {
      PreToolUse: [{
        matcher: 'Bash|Read|mcp__user-tools__Write|NotBash',
        hooks: [{ type: 'command', command: 'guard' }],
      }],
    })
    expect((rewritten.PreToolUse?.[0] as any).matcher).toBe(
      'mcp__user-tools__Bash|mcp__user-tools__Read|mcp__user-tools__Write|NotBash',
    )
  })

  it('rejects new cross-marketplace name collisions without changing the official namespace', () => {
    expect(() => assertUniqueEnabledPluginNames(['sample@alpha', 'sample@zeta']))
      .toThrow('Plugin namespace "sample" conflict: sample@alpha and sample@zeta cannot be enabled together')
    expect(() => assertUniqueEnabledPluginNames(['sample@alpha', 'other@zeta'])).not.toThrow()
  })

  it('chooses a deterministic runtime winner for legacy duplicate enabled entries', () => {
    const plugin = (key: string): InstalledPlugin => {
      const at = key.lastIndexOf('@')
      return {
        name: key.slice(0, at),
        marketplace: key.slice(at + 1),
        key,
        installPath: `/cache/${key}`,
      }
    }
    const duplicates = [plugin('sample@zeta'), plugin('sample@alpha')]

    expect(selectPluginNamespaceWinners(duplicates, new Set()).map(item => item.key))
      .toEqual(['sample@alpha'])
    expect(selectPluginNamespaceWinners(duplicates, new Set(['sample@zeta'])).map(item => item.key))
      .toEqual(['sample@zeta'])
  })

  it('matches the longest configured LSP extension case-insensitively', () => {
    const mapping = { '.php': 'php', '.blade.php': 'blade', ts: 'typescript' }
    expect(matchLspLanguage(mapping, '/repo/view.BLADE.PHP')).toBe('blade')
    expect(matchLspLanguage(mapping, '/repo/main.ts')).toBe('typescript')
    expect(matchLspLanguage(mapping, '/repo/readme.md')).toBeNull()
  })

  it('decodes fragmented and coalesced LSP Content-Length frames', () => {
    const decoder = new LspFrameDecoder()
    const frame = (value: unknown) => {
      const body = Buffer.from(JSON.stringify(value))
      return Buffer.concat([Buffer.from(`Content-Length: ${body.length}\r\n\r\n`), body])
    }
    const first = frame({ jsonrpc: '2.0', id: 1, result: 'one' })
    const second = frame({ jsonrpc: '2.0', method: 'ready' })

    expect(decoder.push(first.subarray(0, 10))).toEqual([])
    expect(decoder.push(Buffer.concat([first.subarray(10), second]))).toEqual([
      { jsonrpc: '2.0', id: 1, result: 'one' },
      { jsonrpc: '2.0', method: 'ready' },
    ])
  })

  it('rejects oversized or malformed LSP frames', () => {
    expect(() => new LspFrameDecoder().push(Buffer.alloc(LSP_MAX_HEADER_BYTES + 1, 65)))
      .toThrow('header exceeded')
    expect(() => new LspFrameDecoder().push(Buffer.from(`Content-Length: ${LSP_MAX_CONTENT_BYTES + 1}\r\n\r\n`)))
      .toThrow('Content-Length exceeded')
    expect(() => new LspFrameDecoder().push(Buffer.alloc(LSP_MAX_STDOUT_BUFFER_BYTES + 1)))
      .toThrow('stdout buffer exceeded')
    expect(() => new LspFrameDecoder().push(Buffer.from('Content-Length: 1\r\n\r\n{')))
      .toThrow('valid JSON')
    expect(() => new LspFrameDecoder().push(Buffer.from('X-Test: nope\r\n\r\n{}')))
      .toThrow('exactly one Content-Length')
  })
})
