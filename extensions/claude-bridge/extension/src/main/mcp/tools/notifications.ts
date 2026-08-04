// PushNotification — MCP tool that surfaces a desktop/in-app notification
// to the user via our existing toast pipeline. Routes through `spawnToast`
// so the focus-tracker decides whether to render in-window (when the main
// window is focused) or as a separate OS-level toast window (when it isn't).

import { spawnToast } from '../../notifications/toast-window'
import type { McpResult, ToolContext } from '../tool-registry'

export async function pushNotification(
  input: Record<string, unknown>,
  context?: ToolContext,
): Promise<McpResult> {
  const message = typeof input.message === 'string' ? input.message : ''
  if (!message) {
    return { content: [{ type: 'text', text: 'Error: message is required' }] }
  }
  if (!context?.tabId) {
    return { content: [{ type: 'text', text: 'Error: no active tab to attach the notification to' }] }
  }
  const title = typeof input.title === 'string' && input.title ? input.title : 'Claude'
  spawnToast({
    kind: 'finished',
    tabId: context.tabId,
    title,
    message,
    sticky: false,
  })
  return { content: [{ type: 'text', text: 'Notification sent.' }] }
}

// TodoWrite — minimal handler so the CLI's reminder system stops complaining
// about "tool not registered". Persists the most recent todo list per tab in
// memory (`todoStore`). Surfacing this list in the UI is left for a follow-up;
// for now it just round-trips back to the model without erroring.

export interface TodoItem {
  content: string
  status: 'pending' | 'in_progress' | 'completed'
  activeForm?: string
}

const todoStore: Map<string, TodoItem[]> = new Map()

export async function todoWrite(
  input: Record<string, unknown>,
  context?: ToolContext,
): Promise<McpResult> {
  const todos = input.todos
  if (!Array.isArray(todos)) {
    return { content: [{ type: 'text', text: 'Error: todos must be an array' }] }
  }
  const key = context?.tabId || '__global__'
  todoStore.set(key, todos as TodoItem[])
  const summary = (todos as TodoItem[]).map((t, i) => `${i + 1}. [${t.status}] ${t.content}`).join('\n')
  return {
    content: [
      {
        type: 'text',
        text: `Todo list updated (${todos.length} item${todos.length === 1 ? '' : 's'}):\n${summary}`,
      },
    ],
  }
}

export function getTodoList(tabId: string): TodoItem[] {
  return todoStore.get(tabId) ?? []
}

// CLI 2.1.121's matching read tool is named `TodoList` (not TodoRead).
// Returns the most recent list as JSON. Source of truth lives in the
// session JSONL — this in-memory store is just a fast path; on PTY restart
// the renderer's `activeTodos` signal re-derives from the JSONL transcript.
export async function todoList(
  _input: Record<string, unknown>,
  context?: ToolContext,
): Promise<McpResult> {
  const key = context?.tabId || '__global__'
  const list = todoStore.get(key) ?? []
  if (list.length === 0) {
    return { content: [{ type: 'text', text: 'No todos.' }] }
  }
  return { content: [{ type: 'text', text: JSON.stringify(list, null, 2) }] }
}
