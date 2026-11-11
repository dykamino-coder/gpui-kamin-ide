import { hostHasTool, hostPlace, hostShells, type HostDeclaration } from './host-declaration'

// Одинаковый текст для initialize и server/discover при одинаковых возможностях клиента.
// Без декларации хоста (старые клиенты) текст прежний; с декларацией — только объявленное.
export function mcpInstructions(tasks = false, host?: HostDeclaration): string {
  const shells = hostShells(host)
  const names = shells.join('/')
  const shell = tasks
    ? `With MCP Tasks support, ${names} foreground calls finish inline within two minutes or return a durable MCP task. run_in_background:true returns a task immediately. The CLI tracks tasks and receives completion notifications; output uses tasks/get and cancellation uses tasks/cancel (MCP protocol methods). Explicit background tasks can run for almost 24 hours; foreground commands keep their timeout (default 25 minutes, maximum 30 minutes).`
    : `Without MCP Tasks support, ${names} run_in_background:true returns a client-side bash_id. Poll BashOutput for new output and use KillShell with bash_id to stop it; there is no automatic completion notification. Native TaskOutput cannot read these legacy shells. Foreground timeout defaults to 25 minutes, maximum 30 minutes.`
  const kinds = ['file', ...(shells.length ? ['shell'] : []), ...(hostHasTool(host, 'WebFetch') ? ['web'] : [])]
  const kindList = kinds.length > 1 ? `${kinds.slice(0, -1).join(', ')} and ${kinds[kinds.length - 1]}` : kinds[0]
  const place = host ? hostPlace(host) : "the user's machine"
  return [
    `user-tools ${kindList} tools run on ${place}; project_sessions_list / project_session_* read Bridge's server-side conversation history. Native tools and other MCP servers keep their own routing. Use each tool's full definition for arguments.`,
    `Text results are capped at about 720,000 characters; some tools have lower limits. A result containing "[Output truncated by bridge" is incomplete: narrow the query and repeat.`,
    ...(shells.length ? [`Shell: ${shell}`] : []),
    ...(hostHasTool(host, 'EnterWorktree')
      ? [
          'Use EnterWorktree only when the user explicitly asks for a worktree; it does not switch the session working directory.',
        ]
      : []),
  ].join('\n')
}
