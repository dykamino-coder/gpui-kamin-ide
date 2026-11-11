import fs from 'fs'
import os from 'os'
import path from 'path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { SyncPluginData } from '../types/pty'
import { writePluginModuleTree } from './plugin-modules'

const tempDirs: string[] = []

function pluginRoot(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-plugin-modules-'))
  tempDirs.push(dir)
  return path.join(dir, 'plugin')
}

function plugin(extra: Partial<SyncPluginData>): SyncPluginData {
  return {
    id: 'mod@market',
    name: 'mod',
    marketplace: 'market',
    sourceRoot: 'C:/plugins/mod',
    manifest: { name: 'mod' },
    skills: {},
    agents: {},
    commands: {},
    workflows: {},
    outputStyles: {},
    themes: {},
    hooks: {},
    ...extra,
  }
}

afterEach(() => {
  vi.unstubAllEnvs()
  for (const dir of tempDirs.splice(0)) fs.rmSync(dir, { recursive: true, force: true })
})

describe('plugin hooks modules', () => {
  it('writes the plugin tree and a hooks.json that names only the modules', async () => {
    const root = pluginRoot()
    const modules = await writePluginModuleTree(
      root,
      plugin({
        modules: ['./register.tsx', '../lib/extra.ts'],
        files: {
          'hooks/register.tsx': { encoding: 'utf8', content: "import '../lib/extra'\n" },
          'lib/extra.ts': { encoding: 'utf8', content: 'export {}\n' },
          'types/index.d.ts': { encoding: 'utf8', content: 'export {}\n' },
          'assets/ping.wav': { encoding: 'base64', content: Buffer.from([0, 255, 128]).toString('base64') },
        },
      }),
    )

    expect(modules).toEqual(['./register.tsx', '../lib/extra.ts'])
    expect(JSON.parse(fs.readFileSync(path.join(root, 'hooks', 'hooks.json'), 'utf-8'))).toEqual({ modules })
    expect(fs.readFileSync(path.join(root, 'hooks', 'register.tsx'), 'utf-8')).toBe("import '../lib/extra'\n")
    expect(fs.existsSync(path.join(root, 'types', 'index.d.ts'))).toBe(true)
    expect([...fs.readFileSync(path.join(root, 'assets', 'ping.wav'))]).toEqual([0, 255, 128])
  })

  it('does not materialize host-side components or paths outside the plugin root', async () => {
    const root = pluginRoot()
    await writePluginModuleTree(
      root,
      plugin({
        modules: ['./register.ts', '../../outside.ts', './missing.ts'],
        files: {
          'hooks/register.ts': { encoding: 'utf8', content: 'export {}\n' },
          'hooks/hooks.json': { encoding: 'utf8', content: '{"hooks":{"Stop":[]}}' },
          '.mcp.json': { encoding: 'utf8', content: '{}' },
          '.lsp.json': { encoding: 'utf8', content: '{}' },
          'monitors/monitors.json': { encoding: 'utf8', content: '[]' },
          '.claude-plugin/plugin.json': { encoding: 'utf8', content: '{"mcpServers":{}}' },
          '../outside.ts': { encoding: 'utf8', content: 'export {}\n' },
        },
      }),
    )

    expect(JSON.parse(fs.readFileSync(path.join(root, 'hooks', 'hooks.json'), 'utf-8'))).toEqual({
      modules: ['./register.ts'],
    })
    for (const rel of ['.mcp.json', '.lsp.json', 'monitors/monitors.json', '.claude-plugin/plugin.json']) {
      expect(fs.existsSync(path.join(root, rel))).toBe(false)
    }
    expect(fs.existsSync(path.join(root, '..', 'outside.ts'))).toBe(false)
  })

  it('writes nothing for a plugin without modules or when mods are switched off', async () => {
    const withoutModules = pluginRoot()
    expect(await writePluginModuleTree(withoutModules, plugin({}))).toEqual([])
    expect(fs.existsSync(withoutModules)).toBe(false)

    vi.stubEnv('BRIDGE_PLUGIN_MODS', '0')
    const switchedOff = pluginRoot()
    const files = { 'hooks/register.ts': { encoding: 'utf8' as const, content: 'export {}\n' } }
    expect(await writePluginModuleTree(switchedOff, plugin({ modules: ['./register.ts'], files }))).toEqual([])
    expect(fs.existsSync(switchedOff)).toBe(false)
  })
})
