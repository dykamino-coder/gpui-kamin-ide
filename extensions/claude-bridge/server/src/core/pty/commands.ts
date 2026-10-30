// Session-scoped projection of the execution owners. Never consumes BashOutput.
import { pendingMcpCalls } from './session-mcp-call'
import { mcpTasks } from './mcp-tasks'
import { sendToClient } from './session-io'
import type { PtySession } from './types'
import type { CommandReport, CommandSnapshot } from '../../shared/commands'

const reports = new Map<string, { report: CommandReport; at: number }>()
const output = new Map<string, string>()
const OUTPUT_CAP = 16 * 1024
const MAX_RETAINED = 256
function owner(session: PtySession): string {
  return session.mcpEndpoint?.id ?? session.id
}
function outputKey(session: PtySession, id: string): string {
  return `${owner(session)}:${id}`
}
function retain(session: PtySession, id: string, text: string): void {
  const key = outputKey(session, id)
  output.delete(key)
  output.set(key, text.slice(-OUTPUT_CAP))
  while (output.size > MAX_RETAINED) output.delete(output.keys().next().value!)
}
export function rememberCommandResult(session: PtySession, id: string, result: unknown): void {
  const content = result && typeof result === 'object' ? (result as { content?: unknown }).content : undefined
  const text = Array.isArray(content)
    ? content
        .filter((part) => part?.type === 'text' && typeof part.text === 'string')
        .map((part) => part.text)
        .join('\n')
    : typeof result === 'string'
      ? result
      : (JSON.stringify(result) ?? '')
  retain(session, id, text)
}
export async function acceptCommandReport(session: PtySession, report: CommandReport): Promise<void> {
  if (!report || !Array.isArray(report.calls) || !Array.isArray(report.shells)) return
  const tasks = await mcpTasks.working()
  for (const call of report.calls.slice(0, 128)) {
    if (typeof call?.id !== 'string' || typeof call.output !== 'string') continue
    if (
      pendingMcpCalls.get(call.id)?.sessionId === session.id ||
      tasks.some((t) => t.owner === owner(session) && t.taskId === call.id)
    )
      retain(session, call.id, call.output)
  }
  const shells = report.shells
    .slice(0, 100)
    .filter(
      (s) =>
        s &&
        /^shell:[0-9]+$/.test(s.id) &&
        typeof s.command === 'string' &&
        typeof s.tool === 'string' &&
        Number.isFinite(s.startedAt) &&
        typeof s.output === 'string',
    )
    .map((s) => ({
      ...s,
      command: s.command.slice(0, 65536),
      description: typeof s.description === 'string' ? s.description.slice(0, 1024) : undefined,
      tool: s.tool.slice(0, 64),
      output: s.output.slice(-OUTPUT_CAP),
    }))
  reports.delete(owner(session))
  reports.set(owner(session), { report: { calls: [], shells }, at: Date.now() })
  // Bounded metadata; active sessions refresh their entries each second.
  while (reports.size > MAX_RETAINED) reports.delete(reports.keys().next().value!)
  for (const shell of shells) retain(session, shell.id, shell.output)
}
export async function commandSnapshot(session: PtySession): Promise<CommandSnapshot> {
  const commands: CommandSnapshot['commands'] = []
  for (const [id, call] of pendingMcpCalls) {
    if (call.sessionId !== session.id) continue
    commands.push({
      id,
      tool: call.toolName,
      command: String(call.input.command ?? call.toolName).slice(0, 65536),
      description: typeof call.input.description === 'string' ? call.input.description.slice(0, 1024) : undefined,
      background: false,
      status: 'running',
      startedAt: call.createdAt,
    })
  }
  for (const task of await mcpTasks.commands(owner(session))) {
    if (task.owner !== owner(session)) continue
    commands.push({
      id: task.taskId,
      tool: task.toolName,
      command: String(task.input.command ?? task.command ?? task.toolName),
      description: task.description,
      finishedAt: task.status === 'working' ? undefined : Date.parse(task.lastUpdatedAt),
      exitCode: typeof task.result?.exitCode === 'number' ? task.result.exitCode : undefined,
      background: !!task.background,
      status:
        task.status === 'working'
          ? task.cancelRequested
            ? 'stopping'
            : 'running'
          : task.status === 'cancelled'
            ? 'stopped'
            : task.status === 'failed' ||
                task.result?.isError ||
                (typeof task.result?.exitCode === 'number' && task.result.exitCode !== 0)
              ? 'failed'
              : 'completed',
      startedAt: Date.parse(task.createdAt),
    })
  }
  const latest = reports.get(owner(session))
  for (const shell of latest?.report.shells ?? []) {
    if (shell.hiddenFromCommands) continue
    commands.push({
      id: shell.id,
      tool: shell.tool,
      command: shell.command,
      description: shell.description,
      finishedAt: shell.finishedAt,
      exitCode: shell.exitCode,
      background: true,
      status: shell.running
        ? Date.now() - latest!.at > 5000
          ? 'disconnected'
          : 'running'
        : shell.stopped
          ? 'stopped'
          : shell.exitCode === 0
            ? 'completed'
            : 'failed',
      startedAt: shell.startedAt,
    })
  }
  commands.sort((a, b) => Number(b.background) - Number(a.background) || a.startedAt - b.startedAt)
  return { sessionId: session.id, commands }
}
export async function commandOutput(session: PtySession, id: string): Promise<string> {
  const task = await mcpTasks.get(owner(session), id)
  if (task?.result) {
    const content = task.result.content as Array<{ type?: string; text?: string }> | undefined
    return (content ?? [])
      .filter((part) => part.type === 'text')
      .map((part) => part.text ?? '')
      .join('\n')
      .slice(-OUTPUT_CAP)
  }
  if (task?.error) return task.error.message
  return output.get(outputKey(session, id)) ?? ''
}
export async function stopCommand(session: PtySession, id: string): Promise<void> {
  const task = await mcpTasks.get(owner(session), id)
  if (task?.status === 'working') {
    await mcpTasks.update(owner(session), id, (t) => {
      t.cancelRequested = true
    })
    sendToClient(session.ws, { type: 'mcp:cancel', requestId: id, taskOwner: owner(session) })
  } else if (pendingMcpCalls.get(id)?.sessionId === session.id) {
    sendToClient(session.ws, { type: 'mcp:cancel', requestId: id })
  } else if (reports.get(owner(session))?.report.shells.some((s) => s.id === id && s.running)) {
    sendToClient(session.ws, { type: 'commands:stop-shell', id, owner: owner(session) })
  }
}

export async function clearCompletedCommands(session: PtySession): Promise<void> {
  await mcpTasks.clearCommands(owner(session))
  for (const shell of reports.get(owner(session))?.report.shells ?? []) {
    if (!shell.running) shell.hiddenFromCommands = true
  }
}
