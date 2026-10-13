// AGENTS.md sync: Bridge reproduces Claude CLI's default instruction rule
// (`claude-md-or-agents-md`, CLI 2.1.277+) because the CLI's cwd is the
// session settingsDir, whose Bridge-written CLAUDE.md always exists.
import 'reflect-metadata'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import fs from 'fs'
import path from 'path'

const syncRoots = vi.hoisted(() => ({
  home: '/tmp/bridge-agents-home-unset',
  user: '/tmp/bridge-agents-user-unset',
  project: '/tmp/bridge-agents-project-unset',
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

import { applySyncData, pickInstructionFile } from './session-settings'

const tempDirs: string[] = []

function tempDir(): string {
  const dir = fs.mkdtempSync(path.join(process.env.TMPDIR || '/tmp', 'bridge-agents-md-'))
  tempDirs.push(dir)
  return dir
}

function write(root: string, rel: string, content: string): void {
  const target = path.join(root, rel)
  fs.mkdirSync(path.dirname(target), { recursive: true })
  fs.writeFileSync(target, content, 'utf-8')
}

const read = (root: string, rel: string) => fs.readFileSync(path.join(root, rel), 'utf-8')
const exists = (root: string, rel: string) => fs.existsSync(path.join(root, rel))

let settingsDir: string

beforeEach(() => {
  const root = tempDir()
  syncRoots.home = path.join(root, 'home')
  syncRoots.user = path.join(root, 'user')
  syncRoots.project = path.join(root, 'project')
  settingsDir = path.join(root, 'session')
  write(settingsDir, 'CLAUDE.md', '# Bridge session\n')
})

afterEach(() => {
  for (const dir of tempDirs.splice(0)) fs.rmSync(dir, { recursive: true, force: true })
})

describe('AGENTS.md sync', () => {
  it('uses project AGENTS.md pair in the CLAUDE.md slots when the project has no CLAUDE.md', () => {
    write(syncRoots.project, 'AGENTS.md', 'agents-root-rule\n')
    write(syncRoots.project, '.claude/AGENTS.md', 'agents-dot-rule\n')

    applySyncData(settingsDir, '0123456789abcdef', '/repo')

    const session = read(settingsDir, 'CLAUDE.md')
    expect(session).toContain('# Project Instructions (synced)\n\nagents-root-rule')
    expect(read(settingsDir, '.claude/CLAUDE.md')).toBe('agents-dot-rule\n')
    // No raw copy: the content already sits where CLAUDE.md would be.
    expect(exists(settingsDir, 'AGENTS.md')).toBe(false)
    expect(exists(settingsDir, '.claude/AGENTS.md')).toBe(false)
  })

  it('keeps both when the project has CLAUDE.md: CLAUDE.md feeds instructions, AGENTS.md is mirrored raw', () => {
    write(syncRoots.project, 'CLAUDE.md', 'claude-root-rule\n')
    write(syncRoots.project, 'AGENTS.md', 'agents-root-rule\n')
    write(syncRoots.project, '.claude/AGENTS.md', 'agents-dot-rule\n')

    applySyncData(settingsDir, '0123456789abcdef', '/repo')

    const session = read(settingsDir, 'CLAUDE.md')
    expect(session).toContain('claude-root-rule')
    expect(session).not.toContain('agents-root-rule')
    expect(read(settingsDir, 'AGENTS.md')).toBe('agents-root-rule\n')
    expect(read(settingsDir, '.claude/AGENTS.md')).toBe('agents-dot-rule\n')
    expect(exists(settingsDir, '.claude/CLAUDE.md')).toBe(false)
  })

  it("a project .claude/CLAUDE.md alone also counts as the project's own CLAUDE.md", () => {
    write(syncRoots.project, '.claude/CLAUDE.md', 'dot-claude-rule\n')
    write(syncRoots.project, 'AGENTS.md', 'agents-root-rule\n')

    applySyncData(settingsDir, '0123456789abcdef', '/repo')

    expect(read(settingsDir, 'CLAUDE.md')).not.toContain('agents-root-rule')
    expect(read(settingsDir, '.claude/CLAUDE.md')).toBe('dot-claude-rule\n')
    expect(read(settingsDir, 'AGENTS.md')).toBe('agents-root-rule\n')
  })

  it('re-apply is idempotent and drops stale raw mirrors after AGENTS.md is removed', () => {
    write(syncRoots.project, 'CLAUDE.md', 'claude-root-rule\n')
    write(syncRoots.project, 'AGENTS.md', 'agents-root-rule\n')
    applySyncData(settingsDir, '0123456789abcdef', '/repo')
    fs.rmSync(path.join(syncRoots.project, 'AGENTS.md'))
    applySyncData(settingsDir, '0123456789abcdef', '/repo')

    expect(exists(settingsDir, 'AGENTS.md')).toBe(false)
    expect(read(settingsDir, 'CLAUDE.md').split('# Project Instructions (synced)').length).toBe(2)
  })

  it('user-level AGENTS.md is used only without a user CLAUDE.md', () => {
    write(syncRoots.user, 'AGENTS.md', 'user-agents-rule\n')
    applySyncData(settingsDir, '0123456789abcdef', undefined)
    expect(read(settingsDir, 'CLAUDE.md')).toContain('# User Instructions (synced)\n\nuser-agents-rule')

    write(syncRoots.user, 'CLAUDE.md', 'user-claude-rule\n')
    applySyncData(settingsDir, '0123456789abcdef', undefined)
    const session = read(settingsDir, 'CLAUDE.md')
    expect(session).toContain('user-claude-rule')
    expect(session).not.toContain('user-agents-rule')
  })

  it('pickInstructionFile treats an empty CLAUDE.md as present', () => {
    const dir = tempDir()
    write(dir, 'CLAUDE.md', '')
    write(dir, 'AGENTS.md', 'fallback')
    expect(pickInstructionFile(dir, 'CLAUDE.md', 'AGENTS.md')).toEqual({ name: 'CLAUDE.md', content: '' })
    fs.rmSync(path.join(dir, 'CLAUDE.md'))
    expect(pickInstructionFile(dir, 'CLAUDE.md', 'AGENTS.md')).toEqual({ name: 'AGENTS.md', content: 'fallback' })
    fs.rmSync(path.join(dir, 'AGENTS.md'))
    expect(pickInstructionFile(dir, 'CLAUDE.md', 'AGENTS.md')).toBeNull()
  })
})
