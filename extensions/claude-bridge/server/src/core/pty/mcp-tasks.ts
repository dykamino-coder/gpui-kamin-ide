// Tasks remain owned by the token's settings directory across PTY replacement.
import { promises as fs } from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import { createHash, createHmac, randomBytes } from 'node:crypto'
import type { PtySession } from './types'
import { notifyTaskChanged } from './mcp-tool-notify'
import { sendToClient } from './session-io'
import { McpTaskStore, taskDeadline, taskView, type TaskRecord } from './mcp-task-store'

const root = path.join(os.homedir(), '.claude', 'bridge-mcp')
export const mcpTasks = new McpTaskStore(path.join(root, 'tasks'))
let keyPromise: Promise<Buffer> | undefined
async function identityKey(): Promise<Buffer> {
  keyPromise ??= (async () => {
    await fs.mkdir(root, { recursive: true, mode: 0o700 })
    const file = path.join(root, 'identity-key')
    try {
      await fs.writeFile(file, randomBytes(32), { flag: 'wx', mode: 0o600 })
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error
    }
    const key = await fs.readFile(file)
    if (key.length !== 32) throw new Error('Invalid MCP identity key')
    return key
  })().catch((error) => {
    keyPromise = undefined
    throw error
  })
  return keyPromise
}
export async function mcpIdentity(tokenId: string, settingsDir: string): Promise<{ id: string; token: string }> {
  const id = createHash('sha256')
    .update(JSON.stringify([tokenId, path.resolve(settingsDir)]))
    .digest('hex')
  return {
    id,
    token: createHmac('sha256', await identityKey())
      .update(id)
      .digest('hex'),
  }
}

let liveSessions: () => PtySession[] = () => []
let timer: ReturnType<typeof setInterval> | undefined
let draining = false
export function startMcpTaskDelivery(sessions: () => PtySession[]): void {
  liveSessions = sessions
  timer ??= setInterval(() => {
    void drainTasks()
  }, 1000)
  timer.unref?.()
}
export function resolveMcpEndpoint(id: string): PtySession | undefined {
  return liveSessions().find((session) => session.mcpEndpoint?.id === id && !session.teardown)
}
async function dispatch(task: TaskRecord, session: PtySession): Promise<void> {
  if (task.cancelRequested) {
    sendToClient(session.ws, { type: 'mcp:cancel', requestId: task.taskId, taskOwner: task.owner })
  } else if (!task.delivered && session.ws.readyState === 1) {
    // Persist the dispatch boundary before sending. A crash never re-executes an
    // admitted side effect; a surviving client can still settle by task ID.
    const admitted = await mcpTasks.update(task.owner, task.taskId, (record) => {
      record.delivered = true
    })
    if (!admitted || admitted.status !== 'working' || admitted.cancelRequested) return
    if (
      !sendToClient(session.ws, {
        type: 'mcp:call',
        requestId: task.taskId,
        toolName: task.toolName,
        input: task.input,
        taskOwner: task.owner,
        ...(task.input.run_in_background === true ? { taskDeadlineMs: taskDeadline(task) } : {}),
      })
    ) {
      await mcpTasks.update(task.owner, task.taskId, (record) => {
        record.delivered = false
      })
    }
  }
}
async function drainTasks(): Promise<void> {
  if (draining) return
  draining = true
  try {
    for (let task of await mcpTasks.working()) {
      if (Date.now() >= taskDeadline(task) + 60_000) {
        const session = resolveMcpEndpoint(task.owner)
        if (session) sendToClient(session.ws, { type: 'mcp:cancel', requestId: task.taskId, taskOwner: task.owner })
        const ended = await mcpTasks.update(task.owner, task.taskId, (record) => {
          record.status = record.cancelRequested ? 'cancelled' : 'failed'
          if (record.status === 'failed')
            record.error = {
              code: -32000,
              message: 'Execution deadline exceeded; command is never re-executed automatically.',
            }
          record.input = {}
        })
        if (session && ended) notifyTaskChanged(session.id, taskView(ended))
        continue
      }
      if (Date.now() >= taskDeadline(task) && !task.cancelRequested) {
        task = (await mcpTasks.update(task.owner, task.taskId, (record) => {
          record.cancelRequested = true
        }))!
      }
      const session = resolveMcpEndpoint(task.owner)
      if (session) await dispatch(task, session)
    }
  } catch {
    /* failed persistence retains outcomes in the client's existing outbox */
  } finally {
    draining = false
  }
}
export async function createMcpTask(
  session: PtySession,
  toolName: string,
  input: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  if (!session.mcpEndpoint) throw new Error('Task identity unavailable')
  const task = await mcpTasks.create(session.mcpEndpoint.id, toolName, input)
  void drainTasks()
  return { ...taskView(task), resultType: 'task' }
}

/** Waits up to `graceMs` for a task to leave `working`; returns the latest record. */
export async function awaitMcpTask(
  owner: string,
  taskId: string,
  graceMs: number,
  signal?: AbortSignal,
): Promise<TaskRecord | undefined> {
  const deadline = Date.now() + graceMs
  for (;;) {
    const task = await mcpTasks.get(owner, taskId)
    if (!task || task.status !== 'working' || Date.now() >= deadline || signal?.aborted) return task
    await new Promise((resolve) => setTimeout(resolve, Math.min(200, Math.max(0, deadline - Date.now()))))
  }
}

export async function settleMcpTask(
  session: PtySession,
  outcome: { requestId: string; taskOwner?: string; type: string; result?: unknown; reason?: string },
): Promise<boolean> {
  if (!outcome.taskOwner || outcome.taskOwner !== session.mcpEndpoint?.id) return false
  const task = await mcpTasks.settle(
    outcome.taskOwner,
    outcome.requestId,
    outcome.result,
    outcome.type === 'mcp:denied' ? outcome.reason || 'Tool denied' : undefined,
  )
  if (task) notifyTaskChanged(session.id, taskView(task))
  return !!task
}
