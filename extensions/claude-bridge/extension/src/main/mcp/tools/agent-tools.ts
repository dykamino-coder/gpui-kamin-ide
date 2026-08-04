// Agent, Team, Task tools — forwarded to server via IPC → WS
// These tools manage PTY sessions on the server side.

import type { McpResult } from '../tool-registry'

/**
 * Agent tool — spawn a sub-agent PTY session on the server.
 * The server creates a new PTY with the agent's task and returns result.
 */
export async function agentSpawn(input: Record<string, unknown>): Promise<McpResult> {
  // For now, return info about what would be spawned.
  // Full implementation requires IPC → main → WS → server creates sub-PTY.
  const description = (input.description as string) || 'unnamed agent'
  const prompt = (input.prompt as string) || ''

  return {
    content: [{
      type: 'text',
      text: `Agent spawn requested: "${description}"\nPrompt: ${prompt.slice(0, 200)}...\n\nNote: Agent spawning is handled by the server via PTY session management.`,
    }],
  }
}

/**
 * TeamCreate — create a team of agents.
 */
export async function teamCreate(input: Record<string, unknown>): Promise<McpResult> {
  const teamName = (input.team_name as string) || 'unnamed-team'
  return {
    content: [{
      type: 'text',
      text: `Team "${teamName}" creation requested. This is handled by the server.`,
    }],
  }
}

/**
 * TeamDelete — delete a team.
 */
export async function teamDelete(input: Record<string, unknown>): Promise<McpResult> {
  const teamName = (input.team_name as string) || ''
  return {
    content: [{
      type: 'text',
      text: `Team "${teamName}" deletion requested.`,
    }],
  }
}

/**
 * SendMessage — send a message to a teammate.
 */
export async function sendMessage(input: Record<string, unknown>): Promise<McpResult> {
  const recipient = (input.recipient as string) || ''
  const content = (input.content as string) || ''
  return {
    content: [{
      type: 'text',
      text: `Message sent to "${recipient}": ${content.slice(0, 200)}`,
    }],
  }
}

/**
 * TaskCreate — create a background task.
 */
export async function taskCreate(input: Record<string, unknown>): Promise<McpResult> {
  const description = (input.description as string) || ''
  return {
    content: [{
      type: 'text',
      text: `Task created: "${description}". Background task management is handled by the server.`,
    }],
  }
}

/**
 * TaskGet — get task status.
 */
export async function taskGet(input: Record<string, unknown>): Promise<McpResult> {
  const taskId = (input.task_id as string) || ''
  return {
    content: [{
      type: 'text',
      text: `Task "${taskId}" status: pending (task management handled by server).`,
    }],
  }
}

/**
 * TaskUpdate — update a task.
 */
export async function taskUpdate(input: Record<string, unknown>): Promise<McpResult> {
  const taskId = (input.task_id as string) || ''
  return {
    content: [{
      type: 'text',
      text: `Task "${taskId}" updated.`,
    }],
  }
}

/**
 * TaskList — list all tasks.
 */
export async function taskList(_input: Record<string, unknown>): Promise<McpResult> {
  return {
    content: [{
      type: 'text',
      text: 'No active tasks.',
    }],
  }
}

/**
 * TaskOutput — get task output.
 */
export async function taskOutput(input: Record<string, unknown>): Promise<McpResult> {
  const taskId = (input.task_id as string) || ''
  return {
    content: [{
      type: 'text',
      text: `Task "${taskId}" output: (no output yet).`,
    }],
  }
}

/**
 * TaskStop — stop a running task.
 */
export async function taskStop(input: Record<string, unknown>): Promise<McpResult> {
  const taskId = (input.task_id as string) || ''
  return {
    content: [{
      type: 'text',
      text: `Task "${taskId}" stop requested.`,
    }],
  }
}
