import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { executeTool } from '../executor'
import { PermissionManager } from '../permission-manager'

vi.mock('vscode', () => ({}))

let home: string
let project: string
let sibling: string

async function write(file: string, text: string) {
  await fs.mkdir(path.dirname(file), { recursive: true })
  await fs.writeFile(file, text)
}
function skill(dir: string, name: string, extra = '') {
  return write(path.join(dir, name, 'SKILL.md'), '---\ndescription: Use ' + name + '\n' + extra + '---\nReturn sample-result for a sample request.')
}
async function catalog(cwd?: string) {
  const result = await executeTool('ListWorkspaceSkills', {}, { cwd, tabId: 'test-tab' })
  return JSON.parse(result.content[0]!.text!) as Array<{ name: string; source: string; description: string; file_path: string }>
}

beforeEach(async () => {
  home = await fs.mkdtemp(path.join(os.tmpdir(), 'bridge-skills-'))
  project = path.join(home, 'project-one')
  sibling = path.join(home, 'project-two')
  vi.spyOn(os, 'homedir').mockReturnValue(home)
})
afterEach(async () => {
  vi.restoreAllMocks()
  await fs.rm(home, { recursive: true, force: true })
})

describe('workspace skill discovery and ordinary Read', () => {
  it('selects the owning project, applies precedence, and returns just the information needed to read the skill', async () => {
    const user = path.join(home, '.claude', 'skills')
    const local = path.join(project, '.claude', 'skills')
    await skill(user, 'common')
    await skill(local, 'common')
    await skill(user, 'hidden')
    await skill(local, 'hidden', 'disable-model-invocation: true\n')
    await skill(local, 'with-hook', 'hooks: {}\n')
    await skill(local, 'limited', 'allowed-tools: Read\n')
    await skill(path.join(sibling, '.claude', 'skills'), 'other')
    const list = await catalog(project)
    expect(list).toEqual([{ name: 'common', source: 'project', description: 'Use common', file_path: path.join(local, 'common', 'SKILL.md') }])
    expect(JSON.stringify(list)).not.toContain('sample-result')
    const read = await executeTool('Read', { file_path: list[0]!.file_path }, { cwd: project })
    expect(read.content[0]!.text).toContain('Return sample-result')
    expect((await catalog()).map(s => s.name)).toEqual(['common', 'hidden'])
    expect((await catalog(sibling)).map(s => s.name)).toEqual(['common', 'hidden', 'other'])
    expect(new PermissionManager().getToolCategory('ListWorkspaceSkills')).toBe('read-only')
  })

  it('follows enabled, current, project-scoped marketplace installations without consulting the focused tab', async () => {
    const first = path.join(home, 'installed', 'v1')
    const second = path.join(home, 'installed', 'v2')
    const plugin = 'sample@fixture'
    const installed = (root: string) => write(path.join(home, '.claude/plugins/installed_plugins.json'), JSON.stringify({
      version: 2, plugins: { [plugin]: [{ scope: 'project', projectPath: project, installPath: root }] },
    }))
    for (const [root, name] of [[first, 'first'], [second, 'second']] as const) {
      await write(path.join(root, '.claude-plugin/plugin.json'), JSON.stringify({ name: 'sample', skills: ['./manual'] }))
      await skill(path.join(root, 'manual'), name)
    }
    await installed(first)
    expect((await catalog(project)).map(s => [s.name, s.source])).toEqual([['first', 'plugin:sample@fixture']])
    expect(await catalog(sibling)).toEqual([])
    expect(await catalog()).toEqual([])
    await installed(second)
    expect((await catalog(project)).map(s => s.name)).toEqual(['second'])
    await write(path.join(home, '.claude/settings.json'), JSON.stringify({ enabledPlugins: { [plugin]: false } }))
    expect(await catalog(project)).toEqual([])
  })
})
