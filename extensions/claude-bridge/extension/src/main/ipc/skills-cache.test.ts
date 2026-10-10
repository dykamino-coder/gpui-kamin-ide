import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const ipcHandlers = vi.hoisted(() => new Map<string, (...args: any[]) => unknown>())

vi.mock('@kaminide/host-compat', () => ({
  ipcMain: {
    handle: (channel: string, handler: (...args: any[]) => unknown) => ipcHandlers.set(channel, handler),
  },
  shell: {
    openPath: vi.fn(),
    showItemInFolder: vi.fn(),
  },
}))

import { invalidateSkillsCache, registerSkillsAgentsIPC } from './skills-agents'

describe('skills:list cache invalidation', () => {
  let root: string
  let project: string

  beforeEach(() => {
    root = fs.mkdtempSync(path.join(os.tmpdir(), 'kamin-skills-cache-'))
    project = path.join(root, 'project')
    const home = path.join(root, 'home')
    fs.mkdirSync(project, { recursive: true })
    fs.mkdirSync(home, { recursive: true })
    vi.spyOn(os, 'homedir').mockReturnValue(home)
    ipcHandlers.clear()
    invalidateSkillsCache()
    registerSkillsAgentsIPC({ getTabManager: () => null, getUserCwd: () => project })
  })

  afterEach(() => {
    vi.restoreAllMocks()
    fs.rmSync(root, { recursive: true, force: true })
  })

  async function invoke<T>(channel: string, ...args: unknown[]): Promise<T> {
    const handler = ipcHandlers.get(channel)
    if (!handler) throw new Error(`Missing IPC handler ${channel}`)
    return await handler({ sender: { send: () => {} } }, ...args) as T
  }

  it('refreshes immediately after create and delete instead of serving the 30s cache', async () => {
    expect(await invoke<any[]>('skills:list')).toEqual([])

    const created = await invoke<{ path: string }>('skills:create', 'fresh-skill', '# Fresh skill')
    expect((await invoke<any[]>('skills:list')).map(row => row.name)).toContain('fresh-skill')

    await invoke('skills:delete', created.path)
    expect((await invoke<any[]>('skills:list')).map(row => row.name)).not.toContain('fresh-skill')
  })

  it('creates a project skill directory, not a legacy command, and lists it as project', async () => {
    const created = await invoke<{ path: string }>('skills:create', 'Review Code', '# Review the diff')

    expect(created.path).toBe(path.join(project, '.claude', 'skills', 'review-code', 'SKILL.md'))
    expect(fs.existsSync(path.join(project, '.claude', 'commands'))).toBe(false)
    const row = (await invoke<any[]>('skills:list')).find(r => r.name === 'review-code')
    expect(row).toMatchObject({ source: 'project', description: 'Review the diff', path: created.path })
  })

  it('creates a user skill instead of writing into the host process cwd when no folder is open', async () => {
    ipcHandlers.clear()
    registerSkillsAgentsIPC({ getTabManager: () => null, getUserCwd: () => null })

    const created = await invoke<{ path: string }>('skills:create', 'no-folder', 'Do it')

    expect(created.path).toBe(path.join(os.homedir(), '.claude', 'skills', 'no-folder', 'SKILL.md'))
    expect((await invoke<any[]>('skills:list')).find(r => r.name === 'no-folder')?.source).toBe('user')
  })

  it('still lists and deletes legacy project commands', async () => {
    const commandsDir = path.join(project, '.claude', 'commands')
    fs.mkdirSync(commandsDir, { recursive: true })
    const legacy = path.join(commandsDir, 'legacy.md')
    fs.writeFileSync(legacy, '# Legacy command')

    expect((await invoke<any[]>('skills:list')).map(row => row.name)).toContain('legacy')
    await invoke('skills:delete', legacy)
    expect(fs.existsSync(legacy)).toBe(false)
  })
})
