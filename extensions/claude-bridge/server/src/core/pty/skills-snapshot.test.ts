import { afterEach, describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { replaceSkillsOverlay } from './skills-snapshot'

const tempDirs: string[] = []

function tempDir(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-skills-snapshot-'))
  tempDirs.push(dir)
  return dir
}

function write(root: string, relativePath: string, content: string): void {
  const target = path.join(root, relativePath)
  fs.mkdirSync(path.dirname(target), { recursive: true })
  fs.writeFileSync(target, content, 'utf-8')
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) fs.rmSync(dir, { recursive: true, force: true })
})

describe('replaceSkillsOverlay', () => {
  it('uses user skills as the base and project skills as the override', () => {
    const root = tempDir()
    const user = path.join(root, 'user')
    const project = path.join(root, 'project')
    const destination = path.join(root, 'session', '.claude', 'skills')
    write(user, 'shared/SKILL.md', 'user')
    write(user, 'user-only/SKILL.md', 'user-only')
    write(project, 'shared/SKILL.md', 'project')
    write(project, 'project-only/SKILL.md', 'project-only')

    replaceSkillsOverlay(destination, user, project)

    expect(fs.readFileSync(path.join(destination, 'shared/SKILL.md'), 'utf-8')).toBe('project')
    expect(fs.readFileSync(path.join(destination, 'user-only/SKILL.md'), 'utf-8')).toBe('user-only')
    expect(fs.readFileSync(path.join(destination, 'project-only/SKILL.md'), 'utf-8')).toBe('project-only')
  })

  it('removes stale files when a complete snapshot deletes a skill', () => {
    const root = tempDir()
    const user = path.join(root, 'user')
    const destination = path.join(root, 'session', '.claude', 'skills')
    write(user, 'removed/SKILL.md', 'old')
    replaceSkillsOverlay(destination, user)
    fs.rmSync(path.join(user, 'removed'), { recursive: true, force: true })

    replaceSkillsOverlay(destination, user)

    expect(fs.existsSync(path.join(destination, 'removed/SKILL.md'))).toBe(false)
  })

  it('lets a project directory replace a colliding user file', () => {
    const root = tempDir()
    const user = path.join(root, 'user')
    const project = path.join(root, 'project')
    const destination = path.join(root, 'session', '.claude', 'skills')
    write(user, 'collision', 'user-file')
    write(project, 'collision/SKILL.md', 'project-directory')

    replaceSkillsOverlay(destination, user, project)

    expect(fs.statSync(path.join(destination, 'collision')).isDirectory()).toBe(true)
    expect(fs.readFileSync(path.join(destination, 'collision/SKILL.md'), 'utf-8')).toBe('project-directory')
  })
})
