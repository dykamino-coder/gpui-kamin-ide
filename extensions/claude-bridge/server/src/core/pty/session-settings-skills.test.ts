import 'reflect-metadata'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import fs from 'fs'
import path from 'path'

const syncRoots = vi.hoisted(() => ({
  home: '/tmp/bridge-settings-home-unset',
  user: '/tmp/bridge-settings-user-unset',
  project: '/tmp/bridge-settings-project-unset',
}))

vi.mock('os', async (importOriginal) => {
  const actual = await importOriginal<typeof import('os')>()
  const mocked = { ...actual, homedir: () => syncRoots.home }
  return { ...mocked, default: mocked }
})

vi.mock('../sync/routes', () => ({
  getUserSyncDir: () => syncRoots.user,
  getProjectSyncDir: () => syncRoots.project,
}))

import { AGENT_TEAMS_REPORTING_CONTRACT } from './bridge-default-claude-md'
import { applySyncData, writeSessionClaudeMd, writeSessionSettings } from './session-settings'

const tempDirs: string[] = []

function tempDir(): string {
  const base = process.env.TMPDIR || '/tmp'
  const dir = fs.mkdtempSync(path.join(base, 'bridge-session-settings-'))
  tempDirs.push(dir)
  return dir
}

function write(root: string, relativePath: string, content: string): void {
  const target = path.join(root, relativePath)
  fs.mkdirSync(path.dirname(target), { recursive: true })
  fs.writeFileSync(target, content, 'utf-8')
}

beforeEach(() => {
  const root = tempDir()
  syncRoots.home = path.join(root, 'home')
  syncRoots.user = path.join(root, 'user')
  syncRoots.project = path.join(root, 'project')
})

afterEach(() => {
  for (const dir of tempDirs.splice(0)) fs.rmSync(dir, { recursive: true, force: true })
})

describe('session settings skills integration', () => {
  it('uses the exact user + project overlay on initial apply and refresh', () => {
    const settingsDir = path.join(tempDir(), 'session')
    write(settingsDir, 'CLAUDE.md', '# Session')
    write(settingsDir, '.claude/skills/stale/SKILL.md', 'stale')
    write(syncRoots.user, 'skills/shared/SKILL.md', 'user')
    write(syncRoots.user, 'skills/user-only/SKILL.md', 'user-only')
    write(syncRoots.project, 'skills/shared/SKILL.md', 'project')
    write(syncRoots.project, 'skills/project-only/SKILL.md', 'project-only')

    applySyncData(settingsDir, '0123456789abcdef', '/repo')

    const destination = path.join(settingsDir, '.claude', 'skills')
    expect(fs.existsSync(path.join(destination, 'stale/SKILL.md'))).toBe(false)
    expect(fs.readFileSync(path.join(destination, 'shared/SKILL.md'), 'utf-8')).toBe('project')
    expect(fs.readFileSync(path.join(destination, 'user-only/SKILL.md'), 'utf-8')).toBe('user-only')
    expect(fs.readFileSync(path.join(destination, 'project-only/SKILL.md'), 'utf-8')).toBe('project-only')
    expect(fs.existsSync(path.join(syncRoots.user, 'skills/project-only/SKILL.md'))).toBe(false)

    fs.rmSync(path.join(syncRoots.project, 'skills'), {
      recursive: true,
      force: true,
    })
    fs.rmSync(path.join(syncRoots.user, 'skills/user-only'), {
      recursive: true,
      force: true,
    })
    applySyncData(settingsDir, '0123456789abcdef', '/repo')

    expect(fs.existsSync(path.join(destination, 'project-only/SKILL.md'))).toBe(false)
    expect(fs.existsSync(path.join(destination, 'user-only/SKILL.md'))).toBe(false)
    expect(fs.readFileSync(path.join(destination, 'shared/SKILL.md'), 'utf-8')).toBe('user')
  })

  it('keeps the Bridge team contract once and before synced instructions', () => {
    const settingsDir = path.join(tempDir(), 'session')
    fs.mkdirSync(settingsDir, { recursive: true })
    writeSessionClaudeMd(settingsDir)
    write(syncRoots.user, 'CLAUDE.md', 'USER RULE')
    write(syncRoots.project, 'CLAUDE.md', 'PROJECT RULE')

    applySyncData(settingsDir, '0123456789abcdef', 'C:\\repo')
    applySyncData(settingsDir, '0123456789abcdef', 'C:\\repo')

    const result = fs.readFileSync(path.join(settingsDir, 'CLAUDE.md'), 'utf-8')
    expect(result.split('## Delegation')).toHaveLength(2)
    expect(result).toContain(AGENT_TEAMS_REPORTING_CONTRACT)
    expect(result.indexOf('## Delegation')).toBeLessThan(result.indexOf('\n# User Instructions (synced)'))
    expect(result.indexOf('\n# User Instructions (synced)')).toBeLessThan(
      result.indexOf('\n# Project Instructions (synced)'),
    )
  })

  it('keeps Agent Teams enabled and the native SendMessage tool available', () => {
    const settingsDir = path.join(tempDir(), 'session')
    fs.mkdirSync(settingsDir, { recursive: true })
    fs.mkdirSync(syncRoots.home, { recursive: true })

    writeSessionSettings('session-id', settingsDir, 'mcp-token')

    const localSettings = JSON.parse(fs.readFileSync(path.join(settingsDir, '.claude', 'settings.local.json'), 'utf-8'))
    expect(localSettings.env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS).toBe('1')
    expect(localSettings.env.CLAUDE_CODE_PROMPT_CACHE_TTL).toBe('1h')
    expect(localSettings.env.CLAUDE_CODE_SUBAGENT_PROMPT_CACHE_TTL).toBe('1h')
    expect(localSettings.env.CLAUDE_CODE_IDLE_COMPACT_MIN_TOKENS).toBe('350000')
    expect(localSettings.env.CLAUDE_CODE_AUTO_COMPACT_WINDOW).toBe('380k')
    expect(localSettings.env.CLAUDE_AUTOCOMPACT_PCT_OVERRIDE).toBe('92')
    expect(localSettings.env.DISABLE_AUTOUPDATER).toBe('1')

    const settings = JSON.parse(fs.readFileSync(path.join(settingsDir, '.claude', 'settings.json'), 'utf-8'))
    expect(settings.permissions.deny).not.toContain('SendMessage')
  })
})
