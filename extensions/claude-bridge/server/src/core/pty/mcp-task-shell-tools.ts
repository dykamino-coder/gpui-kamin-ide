// ============================================================================
// BashOutput / KillShell for background commands that run as MCP tasks.
//
// With MCP Tasks, `Bash`/`PowerShell` with run_in_background:true returns a
// task id instead of the client's numeric bash_id. Agents still follow up with
// BashOutput and KillShell (the tools say what they are for, and a native CLI
// has the same pair for its own background shells). Forwarded to the client,
// such an id named no shell there — or worse, `parseInt` read a leading digit
// of it as some other shell. The server owns the task, so it answers here:
// KillShell cancels the task, BashOutput reports its state and, once it has
// finished, its output.
// ============================================================================

import { cancelMcpTask, mcpTasks } from './mcp-tasks'
import type { TaskRecord } from './mcp-task-store'
import type { PtySession } from './types'

export interface ShellToolResult {
  content: { type: 'text'; text: string }[]
  isError?: boolean
}

/** The task id a BashOutput/KillShell call names, if it can be one. Numeric ids
 *  are the client's own background shells. */
export function taskIdArgument(input: Record<string, unknown>): string | null {
  const raw = input.bash_id ?? input.shell_id ?? input.id ?? input.task_id
  if (typeof raw !== 'string') return null
  const id = raw.trim()
  return id && !/^\d+$/.test(id) && id.length <= 128 && /^[!-~]+$/.test(id) ? id : null
}

function text(value: string, isError = false): ShellToolResult {
  return { content: [{ type: 'text', text: value }], ...(isError ? { isError: true } : {}) }
}

function resultText(task: TaskRecord): string {
  const content = task.result?.content
  if (!Array.isArray(content)) return ''
  return content
    .map((block) =>
      block && typeof block === 'object' && (block as { type?: unknown }).type === 'text'
        ? String((block as { text?: unknown }).text ?? '')
        : '',
    )
    .join('')
}

function describe(task: TaskRecord): ShellToolResult {
  const label = `${task.toolName} task ${task.taskId}`
  switch (task.status) {
    case 'working':
      return text(
        `${label} is still running${task.cancelRequested ? ' (stop requested)' : ''}. Its output arrives with the task's completion notification; KillShell with this id stops it.`,
      )
    case 'completed': {
      const output = resultText(task)
      const failed = task.result?.isError === true
      return text(`${label} finished${failed ? ' with an error' : ''}.${output ? `\n\n${output}` : ' (no output)'}`)
    }
    case 'cancelled':
      return text(`${label} was stopped.`)
    default:
      return text(`${label} failed: ${task.error?.message ?? 'unknown error'}`, true)
  }
}

/** Answers BashOutput/KillShell for an MCP task of this session; null when the
 *  call names no such task and belongs to the client. */
export async function runTaskShellTool(
  session: PtySession,
  toolName: string,
  input: Record<string, unknown>,
  deps: {
    get: (owner: string, id: string) => Promise<TaskRecord | undefined>
    cancel: (session: PtySession, id: string) => Promise<void>
  } = { get: (owner, id) => mcpTasks.get(owner, id), cancel: cancelMcpTask },
): Promise<ShellToolResult | null> {
  if (toolName !== 'BashOutput' && toolName !== 'KillShell') return null
  const owner = session.mcpEndpoint?.id
  const id = taskIdArgument(input)
  if (!owner || !id) return null
  const task = await deps.get(owner, id)
  if (!task || (task.toolName !== 'Bash' && task.toolName !== 'PowerShell')) return null
  if (toolName === 'BashOutput') return describe(task)
  if (task.status !== 'working') return describe(task)
  await deps.cancel(session, id)
  return text(`Stopped ${task.toolName} task ${id}.`)
}
