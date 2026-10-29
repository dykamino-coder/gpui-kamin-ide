// The CLI's system prompt for a remote Bridge session.
//
// Kept in its own module with only dependency-free text imports: session-env pulls in the config and
// proxy layers (and through them a TLS/cert stack), which made this pure string
// builder impossible to unit-test — and it went untested long enough to ship a
// parameter that was accepted and never read.

export function buildSystemPrompt(userCwd?: string, basePrompt?: string, transcriptMirrorDir?: string): string {
  const lines: string[] = []

  lines.push('# Open Claude Bridge — Remote Session')
  lines.push('')

  if (userCwd) {
    lines.push('## Working Directory')
    lines.push(userCwd)
    lines.push('Use this as the project root when resolving relative paths.')
  } else {
    lines.push('No working directory was specified by the user.')
    lines.push('Ask which directory to use before operations that require a project root.')
  }

  // Point Claude at its own full transcript. Your file tools run on the USER's
  // machine (user-tools MCP), and that same machine mirrors the complete
  // conversation — so this path IS readable by your Read/Bash. It holds turns
  // that a /compact has since summarized out of your live context, which is the
  // whole reason to advertise it: it's how you recall earlier history on demand.
  const mirrorDir = transcriptMirrorDir?.trim()
  if (mirrorDir) {
    lines.push('')
    lines.push('## Conversation History (full transcript)')
    lines.push('')
    lines.push(
      `Bridge mirrors conversation history on the user's machine, including turns that a /compact has summarized out of your active context, at:`,
    )
    lines.push(`  ${mirrorDir}`)
    lines.push(
      `This directory is shared by multiple conversations. Each JSONL file is named <conversationId>.jsonl (one JSON entry per line). Use the current conversationId when known and verify that the file belongs to this conversation before reading history; modification time alone does not identify the current session. Read with mcp__user-tools__Read (or Bash/PowerShell). The mirror may still be catching up after reconnect: report missing history rather than assuming it is complete or using another conversation.`,
    )
    lines.push(`It is append-only history: read it, never write to or delete it.`)
  }

  // The user's own standing instructions follow the dynamic session context.
  //
  // This parameter was accepted and then never read: the setting was stored per
  // token, sent on both create and resume, and threaded all the way down here —
  // only to be dropped on the floor. Nothing the user typed ever reached the CLI.
  const trimmed = basePrompt?.trim()
  if (trimmed) {
    lines.push('')
    lines.push('## User Instructions')
    lines.push('')
    lines.push(trimmed)
  }

  return lines.join('\n')
}
