// The CLI's system prompt for a remote Bridge session.
//
// Kept in its own module with NO imports: session-env pulls in the config and
// proxy layers (and through them a TLS/cert stack), which made this pure string
// builder impossible to unit-test — and it went untested long enough to ship a
// parameter that was accepted and never read.

export function buildSystemPrompt(userCwd?: string, basePrompt?: string, transcriptMirrorDir?: string): string {
  const lines: string[] = []

  lines.push('# Open Claude Bridge — Remote Session')
  lines.push('')

  if (userCwd) {
    lines.push(`## Working Directory: ${userCwd}`)
    lines.push('')
    lines.push(`You are working on files located on the USER's machine at: ${userCwd}`)
    lines.push(`When the user asks about "current directory" or "project", they mean ${userCwd}.`)
    lines.push(`Treat ${userCwd} as your primary working directory for all operations.`)
    lines.push(`Always use absolute paths based on ${userCwd} as the project root.`)
  } else {
    lines.push('No working directory was specified by the user.')
    lines.push('Ask the user which directory they want to work in before performing file operations.')
  }

  lines.push('')
  lines.push('## MCP Tool Routing')
  lines.push('')
  lines.push('All file and shell operations execute on the USER\'s local machine via MCP tools (user-tools server), not on this server.')
  lines.push('The MCP tools available are: Read, Write, Edit, Glob, Grep, Bash, NotebookEdit, WebFetch, EnterWorktree, AskUserQuestion. Use the mcp__user-tools__ prefixed versions of these tools for all operations.')
  lines.push('The current server-side directory is a temporary session directory — do NOT access files directly on the server filesystem.')
  lines.push('')
  lines.push('## Interactive Questions')
  lines.push('')
  lines.push('ALWAYS use AskUserQuestion (mcp__user-tools__AskUserQuestion) when asking the user ANY question — quizzes, polls, confirmations, choices, clarifications, or anything that expects a user response. NEVER print questions as plain text. The tool shows an interactive widget with clickable option buttons, which is much better UX than text.')

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
    lines.push(`Your COMPLETE transcript for this session — including earlier turns that a /compact has since summarized out of your active context — is mirrored on the user's machine at:`)
    lines.push(`  ${mirrorDir}`)
    lines.push(`Each conversation is one JSONL file there named <conversationId>.jsonl (one JSON entry per line); the most-recently-modified file is the current session. Read it with mcp__user-tools__Read (or Bash) whenever you need to recall something from earlier in this conversation that is no longer in your context — it runs on the user's machine, where this mirror lives.`)
    lines.push(`It is append-only history: read it, never write to or delete it.`)
  }

  // The user's own standing instructions, LAST so they read as the final word
  // without being able to delete the routing rules above (which are what make a
  // remote session work at all).
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
