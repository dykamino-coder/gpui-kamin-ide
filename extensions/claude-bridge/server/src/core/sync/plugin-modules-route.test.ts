import 'reflect-metadata'
import fs from 'fs'
import path from 'path'
import { afterEach, describe, expect, it, vi } from 'vitest'

vi.hoisted(() => {
  const tmp = (process.env.TMPDIR || process.env.TEMP || '/tmp').replace(/[\\/]$/, '')
  process.env.BRIDGE_SYNC_BASE = `${tmp}/bridge-sync-mods-${process.pid}-${Date.now()}`
})

vi.mock('../auth/tokens', () => ({
  resolveToken: vi.fn(async (token: string) =>
    token === 'owner-secret' ? { tokenId: 'owner-id', userName: 'owner' } : null,
  ),
}))
vi.mock('../pty/session-core', () => ({ getAllSessions: () => [] }))
vi.mock('../pty/session-settings', () => ({ refreshSessionSkills: vi.fn() }))

import { createSyncRoutes, getUserSyncDir, tokenHash } from './routes'

afterEach(() => {
  fs.rmSync(process.env.BRIDGE_SYNC_BASE!, { recursive: true, force: true })
})

describe('user sync route with a hooks-module plugin', () => {
  it('writes a proxy plugin root the CLI can load the module from', async () => {
    const hash = tokenHash('owner-secret')
    const response = await createSyncRoutes().request(`/api/sync/${hash}/user`, {
      method: 'POST',
      headers: { Authorization: 'Bearer owner-secret', 'Content-Type': 'application/json' },
      body: JSON.stringify({
        plugins: {
          'mod@market': {
            id: 'mod@market',
            name: 'mod',
            marketplace: 'market',
            sourceRoot: 'C:/plugins/mod',
            manifest: { name: 'mod', version: '0.1.0' },
            skills: { 'demo/SKILL.md': '# demo\n' },
            agents: {},
            commands: {},
            workflows: {},
            outputStyles: {},
            themes: {},
            hooks: {},
            modules: ['./register.tsx'],
            files: {
              'hooks/register.tsx': { encoding: 'utf8', content: 'export const register = () => {}\n' },
              'lib/extra.ts': { encoding: 'utf8', content: 'export {}\n' },
            },
          },
        },
      }),
    })
    expect(response.status).toBe(200)

    const userDir = getUserSyncDir(hash)
    const index = JSON.parse(fs.readFileSync(path.join(userDir, 'plugins.json'), 'utf-8'))
    expect(index).toHaveLength(1)
    const root = path.join(userDir, 'plugins', index[0].dirName)

    expect(JSON.parse(fs.readFileSync(path.join(root, '.claude-plugin', 'plugin.json'), 'utf-8'))).toEqual({
      name: 'mod',
      version: '0.1.0',
    })
    expect(JSON.parse(fs.readFileSync(path.join(root, 'hooks', 'hooks.json'), 'utf-8'))).toEqual({
      modules: ['./register.tsx'],
    })
    expect(fs.readFileSync(path.join(root, 'hooks', 'register.tsx'), 'utf-8')).toBe(
      'export const register = () => {}\n',
    )
    expect(fs.existsSync(path.join(root, 'lib', 'extra.ts'))).toBe(true)
    expect(fs.readFileSync(path.join(root, 'skills', 'demo', 'SKILL.md'), 'utf-8')).toBe('# demo\n')
  })
})
