import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const ipcHandlers = vi.hoisted(() => new Map<string, (...args: any[]) => unknown>())

vi.mock('electron', () => ({
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

    const created = await invoke<{ fileName: string }>('skills:create', 'fresh-skill', '# Fresh skill')
    expect((await invoke<any[]>('skills:list')).map(row => row.name)).toContain('fresh-skill')

    await invoke('skills:delete', created.fileName)
    expect((await invoke<any[]>('skills:list')).map(row => row.name)).not.toContain('fresh-skill')
  })
})
