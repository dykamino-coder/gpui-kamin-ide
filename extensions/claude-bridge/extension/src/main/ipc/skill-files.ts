import path from 'path'
import fs from 'fs'
import { extractFrontmatter, firstBodyLine } from '../plugin-helpers'

/** Claude Code merged custom slash commands into skills: new entries live in
 *  `<root>/.claude/skills/<name>/SKILL.md`. Legacy `.claude/commands/*.md`
 *  files still load, so they stay listable/deletable but are never created. */
export interface SkillRoots {
  /** `<workspace>/.claude`, or null when no folder is open — the host's own
   *  process cwd is never a project, so it must not receive project files. */
  projectClaudeDir: string | null
  /** `~/.claude`. */
  userClaudeDir: string
}

/** Skill names are lowercase letters, digits and hyphens (max 64 chars). */
export function skillSlug(name: string): string {
  return name.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 64).replace(/-+$/, '')
}

/** The CLI needs `description` frontmatter to decide when the model should
 *  load the skill; plain-markdown instructions from the form get one derived
 *  from their first line. Author-supplied frontmatter is kept untouched. */
export function withSkillFrontmatter(slug: string, content: string): string {
  if (extractFrontmatter(content) !== null) return content
  const description = firstBodyLine(content, slug)
  return `---\nname: ${slug}\ndescription: ${JSON.stringify(description)}\n---\n\n${content.trimStart()}`
}

/** Lexical containment is insufficient: a managed root can redirect into a
 * plugin through a Windows junction. Check every existing ancestor, including
 * dangling links, before creating files or recursively deleting directories. */
function refuseLinkedAncestors(directory: string): void {
  let current = path.resolve(directory)
  for (;;) {
    try {
      if (fs.lstatSync(current).isSymbolicLink()) throw new Error('Linked skill ancestors are not writable')
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
    }
    const parent = path.dirname(current)
    if (parent === current) return
    current = parent
  }
}

export function createSkillFile(roots: SkillRoots, name: string, content: string): { name: string; fileName: string; path: string } {
  const slug = skillSlug(name)
  if (!slug) throw new Error('Skill name must contain letters or digits')
  const skillsDir = path.join(roots.projectClaudeDir ?? roots.userClaudeDir, 'skills')
  const skillDir = path.join(skillsDir, slug)
  refuseLinkedAncestors(skillsDir)
  fs.mkdirSync(skillsDir, { recursive: true })
  try {
    fs.mkdirSync(skillDir)
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'EEXIST') throw new Error(`Skill "${slug}" already exists`)
    throw error
  }
  const filePath = path.join(skillDir, 'SKILL.md')
  fs.writeFileSync(filePath, withSkillFrontmatter(slug, content), 'utf-8')
  return { name: slug, fileName: slug, path: filePath }
}

/** Deletes a project/user skill directory or a legacy project command. Any
 *  other path (plugin content, arbitrary files) is refused. */
export function deleteSkillFile(roots: SkillRoots, skillPath: string): void {
  const target = path.resolve(skillPath)
  const parent = path.dirname(target)
  const skillsRoots = [roots.projectClaudeDir, roots.userClaudeDir]
    .filter((dir): dir is string => dir !== null)
    .map(dir => path.resolve(dir, 'skills'))
  if (path.basename(target).toLowerCase() === 'skill.md' && skillsRoots.includes(path.dirname(parent))) {
    refuseLinkedAncestors(path.dirname(parent))
    // A skill directory itself may be a junction. Unlink it without opening
    // its target; rm's recursive traversal is reserved for our own directory.
    if (fs.lstatSync(parent).isSymbolicLink()) {
      fs.unlinkSync(parent)
      return
    }
    fs.rmSync(parent, { recursive: true, force: true })
    return
  }
  if (roots.projectClaudeDir && target.endsWith('.md') && parent === path.resolve(roots.projectClaudeDir, 'commands')) {
    refuseLinkedAncestors(parent)
    fs.rmSync(target, { force: true })
    return
  }
  throw new Error('Only project or user skills can be deleted')
}
