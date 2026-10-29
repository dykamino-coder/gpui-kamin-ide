// Одинаковый текст для initialize и server/discover при одинаковых возможностях клиента.
export function mcpInstructions(tasks = false): string {
  const shell = tasks
    ? 'With MCP Tasks support, Bash/PowerShell foreground calls finish inline within two minutes or return a durable MCP task. run_in_background:true returns a task immediately. The CLI tracks tasks and receives completion notifications; output uses tasks/get and cancellation uses tasks/cancel (MCP protocol methods). Explicit background tasks can run for almost 24 hours; foreground commands keep their timeout (default 25 minutes, maximum 30 minutes).'
    : 'Without MCP Tasks support, Bash/PowerShell run_in_background:true returns a client-side bash_id. Poll BashOutput for new output and use KillShell with bash_id to stop it; there is no automatic completion notification. Native TaskOutput cannot read these legacy shells. Foreground timeout defaults to 25 minutes, maximum 30 minutes.'
  return `user-tools file, shell and web tools run on the user's machine; project_sessions_list / project_session_* read Bridge's server-side conversation history. Native tools and other MCP servers keep their own routing. Use each tool's full definition for arguments.
Text results are capped at about 720,000 characters; some tools have lower limits. A result containing "[Output truncated by bridge" is incomplete: narrow the query and repeat.
Shell: ${shell}
Use EnterWorktree only when the user explicitly asks for a worktree; it does not switch the session working directory.`
}
