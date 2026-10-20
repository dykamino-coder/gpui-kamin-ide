// Binds the project session tools to an authenticated PtySession: builds the
// caller context from the session (never from tool arguments) and writes the
// access log — caller, tool, outcome, target ref and size, never content.

import os from 'node:os'
import path from 'node:path'
import { infoLog, warnLog } from '../logging'
import { conversationRef, lifecycleLog } from '../logging/lifecycle'
import { getAllSessions } from '../pty/session-manager'
import type { PtySession } from '../pty/types'
import type { CallerContext } from './scope'
import { runProjectSessionTool, type AccessRecord, type McpToolResult, type ProjectSessionToolName } from './tools'

/** Settings and archive modules are loaded on first use: the MCP handler is
 *  imported by light-weight tests that never run these tools. */
export async function callerContext(session: PtySession): Promise<CallerContext> {
  const { SESSIONS_BASE, sanitizeDirName } = await import('../pty/session-settings')
  const { archiveRoot } = await import('../pty/transcript-archive')
  const live = getAllSessions()
    .filter((s) => s.tokenId === session.tokenId && s.state !== 'exited')
    .map((s) => ({
      ids: [s.cliConversationId, ...(s.conversationAliases ?? [])].filter((v): v is string => !!v),
      attached: !s.detachedAt && s.ws?.readyState === 1,
    }))
  let archive: string | null
  try {
    archive = archiveRoot()
  } catch {
    archive = null
  }
  return {
    tokenId: session.tokenId,
    cwd: session.cwd || '',
    settingsDir: session.settingsDir,
    selfIds: [session.cliConversationId, ...(session.conversationAliases ?? [])].filter((v): v is string => !!v),
    live,
    sessionsBase: SESSIONS_BASE,
    projectsDir: path.join(os.homedir(), '.claude', 'projects'),
    archiveRoot: archive,
    sanitizeDirName,
  }
}

export async function runForSession(
  session: PtySession,
  name: ProjectSessionToolName,
  args: Record<string, unknown>,
): Promise<McpToolResult> {
  const log = (r: AccessRecord): void => {
    const ref = r.conversationId ? conversationRef(r.conversationId) : undefined
    const fields = {
      user: session.userName,
      tokenId: session.tokenId,
      sessionId: session.id,
      tool: r.tool,
      outcome: r.outcome,
      conversationRef: ref,
      self: r.self,
      bytes: r.bytes,
      entries: r.entries,
      error: r.error,
    }
    if (r.outcome === 'ok') infoLog('Project session access', fields)
    else warnLog('Project session access refused', fields)
    lifecycleLog('project_session_access', {
      sessionId: session.id,
      tokenId: session.tokenId,
      tool: r.tool,
      outcome: r.outcome,
      conversationId: r.conversationId,
      self: r.self,
      bytes: r.bytes,
      entries: r.entries,
    })
  }
  return runProjectSessionTool(name, args, await callerContext(session), log)
}
