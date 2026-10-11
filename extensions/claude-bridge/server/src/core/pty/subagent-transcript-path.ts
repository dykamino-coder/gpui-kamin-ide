// Resolve an explicit export only inside this authenticated conversation's
// subagent directory. Never accept a user-supplied path or follow an escape link.
import path from 'node:path'
import fsp from 'node:fs/promises'

function contains(root: string, file: string): boolean {
  const relative = path.relative(root, file)
  return relative !== '' && relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative)
}
export async function subagentTranscriptPath(
  mainPath: string | null,
  agentId: unknown,
  aliases: string[] = [],
): Promise<string | null> {
  if (typeof agentId !== 'string') return null
  const id = agentId.replace(/^agent-/, '')
  if (!mainPath || !/^[a-zA-Z0-9_-]+$/.test(id) || id.length > 128) return null
  const slug = path.dirname(mainPath)
  const conversations = [
    path.basename(mainPath, '.jsonl'),
    ...aliases.filter((value) => /^[a-zA-Z0-9_-]+$/.test(value)),
  ]
  for (const conversation of conversations) {
    try {
      const root = path.join(slug, conversation, 'subagents')
      const realSlug = await fsp.realpath(slug)
      const realRoot = await fsp.realpath(root)
      const file = await fsp.realpath(path.join(root, `agent-${id}.jsonl`))
      if (contains(realSlug, realRoot) && contains(realRoot, file)) return file
    } catch {
      /* absent in this compact generation; try a session-owned alias */
    }
  }
  return null
}
