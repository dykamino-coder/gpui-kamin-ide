// The user's synced CLAUDE.md / AGENTS.md as one text for a ChatGPT (codex)
// chat. A Claude session gets them in its session CLAUDE.md (applySyncData);
// codex reads neither that file nor the synced copies (its working directory
// is the session settings dir), so they travel in its developer instructions,
// with the same files and precedence: the user's CLAUDE.md, else AGENTS.md;
// the project's CLAUDE.md pair, else its AGENTS.md pair.

import fs from 'fs'
import path from 'path'
import { warnLog } from '../logging'
import { getProjectSyncDir, getUserSyncDir } from '../sync/storage'

function block(title: string, content: string | undefined): string[] {
  const text = content?.trim()
  return text ? [`# ${title}\n\n${text}`] : []
}

function read(file: string): string | undefined {
  return fs.existsSync(file) ? fs.readFileSync(file, 'utf-8') : undefined
}

/** CLAUDE.md when it exists (even empty), else AGENTS.md: pickInstructionFile's rule. */
function primaryOrFallback(dir: string): string | undefined {
  const claude = path.join(dir, 'CLAUDE.md')
  return fs.existsSync(claude) ? read(claude) : read(path.join(dir, 'AGENTS.md'))
}

export function syncedInstructions(tokenHash: string | undefined, userCwd: string | undefined): string {
  if (!tokenHash) return ''
  try {
    const parts = block('User Instructions (synced)', primaryOrFallback(getUserSyncDir(tokenHash)))
    if (userCwd) {
      const projDir = getProjectSyncDir(tokenHash, userCwd)
      const hasClaudeMd =
        fs.existsSync(path.join(projDir, 'CLAUDE.md')) || fs.existsSync(path.join(projDir, '.claude', 'CLAUDE.md'))
      const name = hasClaudeMd ? 'CLAUDE.md' : 'AGENTS.md'
      parts.push(...block('Project Instructions (synced)', read(path.join(projDir, name))))
      parts.push(...block('Project Instructions (synced, .claude)', read(path.join(projDir, '.claude', name))))
    }
    return parts.join('\n\n')
  } catch (err) {
    // As for a Claude session: unreadable sync data never blocks the chat.
    warnLog('[sync] Synced instructions unavailable for a ChatGPT chat', { error: String(err) })
    return ''
  }
}
