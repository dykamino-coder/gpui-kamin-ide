// Out-of-the-box hook templates. User installs by clicking "Install" in the
// Library tab — bridge writes the entry into ~/.claude/settings.json:hooks.
// Each template carries its source (us) + visible explanation so users
// can see what they're enabling before approving.

import type { HookEvent, HookHandler } from './types'

export interface HookLibraryTemplate {
  id: string
  title: string
  description: string
  event: HookEvent
  matcher?: string
  handler: HookHandler
  /** Why this template is useful — shown in the Library card. */
  rationale: string
  category: 'formatting' | 'notification' | 'safety' | 'integration' | 'archive'
  /** Recommend host even if user picks 'auto'. UI uses for hint. */
  recommendedHost?: 'local' | 'server'
}

export const HOOK_LIBRARY: HookLibraryTemplate[] = [
  {
    id: 'prettier-on-save',
    title: 'Auto-format with Prettier',
    description: 'Runs `prettier --write` after every Edit/Write tool call.',
    event: 'PostToolUse',
    matcher: 'Edit|Write',
    handler: {
      type: 'command',
      command: 'p=$(cat); f=$(printf "%s" "$p" | jq -r ".tool_input.file_path // empty"); [ -z "$f" ] || prettier --write "$f" 2>/dev/null || true',
      shell: 'bash',
      timeout: 30,
      host: 'local',
    },
    rationale: 'Keeps your code base consistently formatted. Prettier picks up your project config automatically.',
    category: 'formatting',
    recommendedHost: 'local',
  },
  {
    id: 'eslint-fix-on-save',
    title: 'Auto-fix ESLint issues',
    description: 'Runs `eslint --fix` after every Edit/Write tool call.',
    event: 'PostToolUse',
    matcher: 'Edit|Write',
    handler: {
      type: 'command',
      command: 'p=$(cat); f=$(printf "%s" "$p" | jq -r ".tool_input.file_path // empty"); [ -z "$f" ] || eslint --fix "$f" 2>/dev/null || true',
      shell: 'bash',
      timeout: 30,
      host: 'local',
    },
    rationale: 'Surfaces lint issues immediately and auto-fixes the safe ones.',
    category: 'formatting',
    recommendedHost: 'local',
  },
  {
    id: 'desktop-notify-on-stop',
    title: 'Completion message on Stop',
    description: 'Adds a visible hook message when Claude finishes a turn.',
    event: 'Stop',
    handler: {
      type: 'command',
      command: 'echo "Claude finished"',
      shell: 'bash',
      host: 'local',
    },
    rationale: 'Makes completion visible in the conversation hook output. Useful on long-running tool chains.',
    category: 'notification',
    recommendedHost: 'local',
  },
  {
    id: 'backup-jsonl-precompact',
    title: 'Backup transcript before Compact',
    description: 'Copies the server-side session JSONL into ~/.claude/backups/ before CLI compacts it.',
    event: 'PreCompact',
    handler: {
      type: 'command',
      command: 'p=$(cat); t=$(printf "%s" "$p" | jq -r ".transcript_path // empty"); [ -z "$t" ] || { mkdir -p ~/.claude/backups && cp "$t" ~/.claude/backups/$(date +%Y%m%d-%H%M%S).jsonl; }',
      shell: 'bash',
      host: 'server',
    },
    rationale: 'Compaction is irreversible — keeping the original transcript lets you recover full history if needed.',
    category: 'archive',
    recommendedHost: 'server',
  },
  {
    id: 'open-worktree-in-vscode',
    title: 'Open worktree in VS Code',
    description: 'Auto-opens a new git worktree in VS Code after Claude creates it.',
    event: 'WorktreeCreate',
    handler: {
      type: 'command',
      command: 'p=$(cat | jq -r ".path // empty" 2>/dev/null); [ -n "$p" ] && code "$p" || true',
      shell: 'bash',
      host: 'local',
    },
    rationale: 'Saves you the manual cd+open step every time Claude branches off into an isolated worktree.',
    category: 'integration',
    recommendedHost: 'local',
  },
  {
    id: 'block-rm-rf-root',
    title: 'Block dangerous Bash patterns',
    description: 'Refuses Bash commands matching `rm -rf /`, `sudo rm`, `:(){ :|:& };:` and similar.',
    event: 'PreToolUse',
    matcher: 'Bash',
    handler: {
      type: 'prompt',
      prompt: `You are a safety classifier. Examine this Bash command and return JSON {"decision":"block"|"approve","reason":"..."}.\n\nDangerous patterns to block: \`rm -rf /\`, \`rm -rf ~\`, \`sudo rm\`, \`dd of=/dev/\`, fork-bombs, format commands, \`> /dev/sda\`. Approve safe operations.\n\nCommand: $TOOL_INPUT`,
      timeout: 10,
    },
    rationale: 'Last-line defense before destructive shell commands run. Uses a small fast model for sub-second decision.',
    category: 'safety',
    recommendedHost: 'server',
  },
  {
    id: 'log-permission-requests',
    title: 'Audit permission requests',
    description: 'Append every PermissionRequest event to ~/.claude/audit.log.',
    event: 'PermissionRequest',
    handler: {
      type: 'command',
      command: 'p=$(cat); n=$(printf "%s" "$p" | jq -r ".tool_name // \"unknown\""); echo "$(date -Iseconds) PERMISSION $n" >> ~/.claude/audit.log',
      shell: 'bash',
      host: 'local',
    },
    rationale: 'Compliance/visibility — see which tools hit the permission gate and when.',
    category: 'safety',
    recommendedHost: 'local',
  },
  {
    id: 'slack-notify-on-stop',
    title: 'Slack ping on Stop',
    description: 'POSTs a Slack webhook when Claude finishes. Set SLACK_WEBHOOK env in bridge config first.',
    event: 'Stop',
    handler: {
      type: 'http',
      url: '${SLACK_WEBHOOK}',
      headers: { 'Content-Type': 'application/json' },
      allowedEnvVars: ['SLACK_WEBHOOK'],
      timeout: 10,
    },
    rationale: 'Centralized team-wide visibility — useful for shared agents on long jobs.',
    category: 'integration',
    recommendedHost: 'server',
  },
  {
    id: 'session-start-load-todos',
    title: 'Inject project TODOs on SessionStart',
    description: 'Reads TODO.md from the client project root and prepends it as additionalContext.',
    event: 'SessionStart',
    handler: {
      type: 'command',
      command: '[ -f ./TODO.md ] && jq -n --arg c "$(cat ./TODO.md)" \'{hookSpecificOutput: {hookEventName: "SessionStart", additionalContext: $c}}\' || true',
      shell: 'bash',
      host: 'local',
    },
    rationale: 'Surface your project TODOs to Claude automatically — no manual /add-context.',
    category: 'integration',
    recommendedHost: 'local',
  },
  {
    id: 'task-completed-log',
    title: 'Log completed tasks',
    description: 'Appends every TaskCompleted event to ~/.claude/tasks.log with a timestamp.',
    event: 'TaskCompleted',
    handler: {
      type: 'command',
      command: 'p=$(cat); s=$(printf "%s" "$p" | jq -r ".task_subject // .task_id // empty" 2>/dev/null); echo "$(date -Iseconds) DONE $s" >> ~/.claude/tasks.log',
      shell: 'bash',
      host: 'local',
    },
    rationale: 'Running history of what the agent finished — handy for standups and progress review.',
    category: 'integration',
    recommendedHost: 'local',
  },
]

export function findTemplate(id: string): HookLibraryTemplate | undefined {
  return HOOK_LIBRARY.find(t => t.id === id)
}
