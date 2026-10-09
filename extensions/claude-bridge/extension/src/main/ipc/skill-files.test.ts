import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { createSkillFile, deleteSkillFile, skillSlug, withSkillFrontmatter, type SkillRoots } from './skill-files'

describe('skill files', () => {
  let root: string
  let roots: SkillRoots

  beforeEach(() => {
    root = fs.mkdtempSync(path.join(os.tmpdir(), 'kamin-skill-files-'))
    roots = { projectClaudeDir: path.join(root, 'project', '.claude'), userClaudeDir: path.join(root, 'home', '.claude') }
  })

  afterEach(() => {
    fs.rmSync(root, { recursive: true, force: true })
  })

  it('normalizes names to the skill naming rules', () => {
    expect(skillSlug('  Review Code!  ')).toBe('review-code')
    expect(skillSlug('my_skill.v2')).toBe('my-skill-v2')
    expect(skillSlug('a'.repeat(63) + '-b')).toBe('a'.repeat(63))
    expect(skillSlug('!!!')).toBe('')
  })

  it('adds name/description frontmatter only when the author did not supply it', () => {
    expect(withSkillFrontmatter('review', '# Review "the" diff\n\nSteps')).toBe(
      '---\nname: review\ndescription: "Review \\"the\\" diff"\n---\n\n# Review "the" diff\n\nSteps',
    )
    const authored = '---\nname: custom\ndescription: Mine\n---\nBody'
    expect(withSkillFrontmatter('review', authored)).toBe(authored)
  })

  it('refuses to overwrite an existing skill or accept an empty name', () => {
    createSkillFile(roots, 'dup', 'one')
    expect(() => createSkillFile(roots, 'dup', 'two')).toThrow('already exists')
    expect(fs.readFileSync(path.join(roots.projectClaudeDir!, 'skills', 'dup', 'SKILL.md'), 'utf-8')).toContain('one')
    expect(() => createSkillFile(roots, '???', 'x')).toThrow()
  })

  it('deletes the whole skill directory but refuses paths outside managed roots', () => {
    const { path: skillMd } = createSkillFile(roots, 'gone', 'x')
    fs.writeFileSync(path.join(path.dirname(skillMd), 'helper.sh'), '')
    deleteSkillFile(roots, skillMd)
    expect(fs.existsSync(path.dirname(skillMd))).toBe(false)

    const pluginSkill = path.join(root, 'plugins', 'p', 'skills', 's', 'SKILL.md')
    fs.mkdirSync(path.dirname(pluginSkill), { recursive: true })
    fs.writeFileSync(pluginSkill, 'x')
    expect(() => deleteSkillFile(roots, pluginSkill)).toThrow()
    expect(() => deleteSkillFile(roots, path.join(roots.projectClaudeDir!, 'settings.json'))).toThrow()
    expect(fs.existsSync(pluginSkill)).toBe(true)
  })

  it.each(['project', 'user'] as const)('refuses create/delete through a %s skills root junction', scope => {
    const claudeDir = scope === 'project' ? roots.projectClaudeDir! : roots.userClaudeDir
    const pluginSkills = path.join(root, 'plugins', 'p', 'skills')
    const victim = path.join(pluginSkills, 'victim')
    fs.mkdirSync(victim, { recursive: true })
    fs.writeFileSync(path.join(victim, 'SKILL.md'), 'plugin')
    fs.writeFileSync(path.join(victim, 'helper.txt'), 'keep')
    fs.mkdirSync(claudeDir, { recursive: true })
    fs.symlinkSync(pluginSkills, path.join(claudeDir, 'skills'), 'junction')
    const effectiveRoots = scope === 'user' ? { ...roots, projectClaudeDir: null } : roots
    expect(() => deleteSkillFile(effectiveRoots, path.join(claudeDir, 'skills', 'victim', 'SKILL.md'))).toThrow('Linked')
    expect(() => createSkillFile(effectiveRoots, 'new', 'x')).toThrow('Linked')
    expect(fs.readFileSync(path.join(victim, 'helper.txt'), 'utf8')).toBe('keep')
    expect(fs.readFileSync(path.join(victim, 'SKILL.md'), 'utf8')).toBe('plugin')
    expect(fs.existsSync(path.join(pluginSkills, 'new'))).toBe(false)
  })

  it('unlinks a skill junction and preserves its external target', () => {
    const external = path.join(root, 'external')
    fs.mkdirSync(external)
    fs.writeFileSync(path.join(external, 'SKILL.md'), 'keep')
    const skills = path.join(roots.projectClaudeDir!, 'skills')
    fs.mkdirSync(skills, { recursive: true })
    fs.symlinkSync(external, path.join(skills, 'linked'), 'junction')
    deleteSkillFile(roots, path.join(skills, 'linked', 'SKILL.md'))
    expect(fs.existsSync(path.join(skills, 'linked'))).toBe(false)
    expect(fs.readFileSync(path.join(external, 'SKILL.md'), 'utf8')).toBe('keep')
  })

  it('preserves plugin content linked inside our own skill directory', () => {
    const { path: skillMd } = createSkillFile(roots, 'own', 'x')
    const plugin = path.join(root, 'plugin')
    fs.mkdirSync(plugin)
    fs.writeFileSync(path.join(plugin, 'helper.txt'), 'keep')
    fs.symlinkSync(plugin, path.join(path.dirname(skillMd), 'helpers'), 'junction')
    deleteSkillFile(roots, skillMd)
    expect(fs.readFileSync(path.join(plugin, 'helper.txt'), 'utf8')).toBe('keep')
  })

  it('refuses legacy command deletion through a junction', () => {
    const plugin = path.join(root, 'plugin')
    fs.mkdirSync(plugin)
    fs.writeFileSync(path.join(plugin, 'legacy.md'), 'keep')
    fs.mkdirSync(roots.projectClaudeDir!, { recursive: true })
    fs.symlinkSync(plugin, path.join(roots.projectClaudeDir!, 'commands'), 'junction')
    expect(() => deleteSkillFile(roots, path.join(roots.projectClaudeDir!, 'commands', 'legacy.md'))).toThrow('Linked')
    expect(fs.readFileSync(path.join(plugin, 'legacy.md'), 'utf8')).toBe('keep')
  })
})
