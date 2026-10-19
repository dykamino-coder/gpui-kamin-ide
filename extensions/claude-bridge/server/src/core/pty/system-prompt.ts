// The CLI's system prompt for a remote Bridge session.
//
// Kept in its own module with only dependency-free text imports: session-env pulls in the config and
// proxy layers (and through them a TLS/cert stack), which made this pure string
// builder impossible to unit-test — and it went untested long enough to ship a
// parameter that was accepted and never read.
import { RUNTIME_SETTINGS_CONTEXT } from './runtime-settings-context'

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
  lines.push(
    "All file and shell operations execute on the USER's local machine via MCP tools (user-tools server), not on this server.",
  )
  lines.push(
    'Use the mcp__user-tools__ prefixed file and shell tools exposed in this session: for example Read, Write, Edit, Glob, Grep, Bash and PowerShell. For client background shells use BashOutput and KillShell with the returned bash_id; native TaskOutput does not track these processes. Other native CLI tools, such as Agent and SendMessage, retain their own routing.',
  )
  lines.push(
    "The current server-side directory is a Bridge-managed session directory, not the user's project directory — do NOT access files directly on the server filesystem.",
  )
  lines.push('')
  lines.push(RUNTIME_SETTINGS_CONTEXT)
  lines.push('')
  lines.push('## Interactive Questions')
  lines.push('')
  lines.push(
    'ALWAYS use AskUserQuestion (mcp__user-tools__AskUserQuestion) when asking the user ANY question — quizzes, polls, confirmations, choices, clarifications, or anything that expects a user response. NEVER print questions as plain text. The tool shows an interactive widget with clickable option buttons, which is much better UX than text.',
  )

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
