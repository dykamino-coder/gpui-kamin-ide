import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { removeMarketplace } from './remove'

const fixture = vi.hoisted(() => ({ home: '' }))
vi.mock('os', async original => {
  const actual = await original<typeof import('node:os')>()
  return { ...actual, default: { ...actual, homedir: () => fixture.home } }
})

let root = ''
const plugins = () => path.join(root, '.claude', 'plugins')
const known = () => JSON.parse(fs.readFileSync(path.join(plugins(), 'known_marketplaces.json'), 'utf8')) as Record<string, unknown>

beforeEach(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), 'marketplace-remove-'))
  fixture.home = root
  fs.mkdirSync(plugins(), { recursive: true })
})
afterEach(() => {
  const resolved = path.resolve(root)
  if (path.dirname(resolved) !== path.resolve(os.tmpdir()) || !path.basename(resolved).startsWith('marketplace-remove-')) throw new Error('Unexpected fixture cleanup path')
  fs.rmSync(resolved, { recursive: true, force: true })
})

describe('removeMarketplace', () => {
  it('forgets a directory marketplace without deleting the user folder', () => {
    const folder = path.join(root, 'my-market')
    fs.mkdirSync(path.join(folder, '.claude-plugin'), { recursive: true })
    fs.writeFileSync(path.join(folder, '.claude-plugin', 'marketplace.json'), '{}')
    const installPath = path.join(plugins(), 'cache', 'local', 'demo', '1.0.0')
    fs.mkdirSync(installPath, { recursive: true })
    fs.writeFileSync(path.join(plugins(), 'known_marketplaces.json'), JSON.stringify({ local: { source: { source: 'directory', path: folder }, installLocation: folder } }))
    fs.writeFileSync(path.join(plugins(), 'installed_plugins.json'), JSON.stringify({ version: 2, plugins: { 'demo@local': [{ installPath }], 'other@else': [{ installPath: root }] } }))

    removeMarketplace('local')

    expect(known()).toEqual({})
    expect(fs.existsSync(path.join(folder, '.claude-plugin', 'marketplace.json'))).toBe(true)
    expect(fs.existsSync(installPath)).toBe(false)
    expect(Object.keys(JSON.parse(fs.readFileSync(path.join(plugins(), 'installed_plugins.json'), 'utf8')).plugins)).toEqual(['other@else'])
  })

  it('deletes the clone of a git marketplace', () => {
    const clone = path.join(plugins(), 'marketplaces', 'remote')
    fs.mkdirSync(clone, { recursive: true })
    fs.writeFileSync(path.join(plugins(), 'known_marketplaces.json'), JSON.stringify({ remote: { source: { source: 'git', url: 'https://example.invalid/m.git' }, installLocation: clone } }))

    removeMarketplace('remote')

    expect(known()).toEqual({})
    expect(fs.existsSync(clone)).toBe(false)
  })
})
