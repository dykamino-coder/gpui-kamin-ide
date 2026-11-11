import fs from 'fs'
import os from 'os'
import path from 'path'
import { afterEach, describe, expect, it } from 'vitest'
import { collectPluginModuleTree } from './plugin-modules'

const tempDirs: string[] = []

function pluginWith(files: Record<string, string | Buffer>): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-plugin-modules-'))
  tempDirs.push(root)
  for (const [rel, content] of Object.entries(files)) {
    const full = path.join(root, rel)
    fs.mkdirSync(path.dirname(full), { recursive: true })
    fs.writeFileSync(full, content)
  }
  return root
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) fs.rmSync(dir, { recursive: true, force: true })
})

describe('collectPluginModuleTree', () => {
  it('returns nothing for a plugin without hooks modules', async () => {
    const root = pluginWith({
      'hooks/hooks.json': JSON.stringify({ hooks: { Stop: [] } }),
      'scripts/run.js': 'process.exit(0)\n',
    })
    expect(await collectPluginModuleTree(root, {})).toBeUndefined()
  })

  it('carries the whole tree of a mod except host-side components', async () => {
    const root = pluginWith({
      '.claude-plugin/plugin.json': JSON.stringify({ name: 'mod', types: './types/index.d.ts' }),
      'hooks/hooks.json': JSON.stringify({ modules: ['./register.tsx'], hooks: { Stop: [] } }),
      'hooks/register.tsx': "import '../lib/extra'\n",
      'lib/extra.ts': 'export {}\n',
      'types/index.d.ts': 'export {}\n',
      'assets/ping.wav': Buffer.from([0, 255, 128]),
      '.mcp.json': '{}',
      'monitors/monitors.json': '[]',
      'skills/demo/SKILL.md': '# demo\n',
      '.git/HEAD': 'ref: refs/heads/main\n',
    })

    const tree = await collectPluginModuleTree(root, {})

    expect(tree?.modules).toEqual(['./register.tsx'])
    expect(Object.keys(tree!.files).sort()).toEqual([
      'assets/ping.wav',
      'hooks/register.tsx',
      'lib/extra.ts',
      'types/index.d.ts',
    ])
    expect(tree!.files['hooks/register.tsx']).toEqual({ encoding: 'utf8', content: "import '../lib/extra'\n" })
    expect(tree!.files['assets/ping.wav']).toEqual({
      encoding: 'base64',
      content: Buffer.from([0, 255, 128]).toString('base64'),
    })
  })

  it('expresses a module declared in a custom hooks file relative to hooks/', async () => {
    const root = pluginWith({
      'config/mod-hooks.json': JSON.stringify({ modules: ['../src/register.ts', '../../escape.ts'] }),
      'src/register.ts': 'export {}\n',
    })
    const tree = await collectPluginModuleTree(root, { hooks: './config/mod-hooks.json' })
    expect(tree?.modules).toEqual(['../src/register.ts'])
  })

  it('leaves an oversized plugin out as a whole and reports why', async () => {
    const root = pluginWith({
      'hooks/hooks.json': JSON.stringify({ modules: ['./register.ts'] }),
      'hooks/register.ts': 'export {}\n',
      'assets/big.bin': Buffer.alloc(512 * 1024 + 1),
    })
    const reasons: string[] = []
    expect(await collectPluginModuleTree(root, {}, (reason) => reasons.push(reason))).toBeUndefined()
    expect(reasons).toEqual(['assets/big.bin is larger than 524288 bytes'])
  })
})
