import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { parseTaskNotification } from './task-notification'
import { CollapsibleBlock } from './CollapsibleBlock'
import { JsonlToolResult } from './JsonlToolResult'

export function TaskNotificationEntry({ text }: { text: string }): JSX.Element {
  const [open, setOpen] = useState(false)
  const task = parseTaskNotification(text)
  if (!task) return <div class="task-notification"><CollapsibleBlock label="task-notification" content={text} color="var(--text-secondary)" /></div>
  return (
    <div class="task-notification">
      <button class="task-notification-header" aria-expanded={open} onClick={() => setOpen(!open)}>
        <i class="fas fa-list-check" aria-hidden="true" />
        <span class="task-notification-summary">{task.summary}</span>
        <span class={`task-notification-status ${task.status}`}>{task.status}</span>
        <small>{task.taskId}</small>
        <i class={`fas fa-chevron-${open ? 'down' : 'right'}`} aria-hidden="true" />
      </button>
      {open && <div class="task-notification-details">
        {task.result !== undefined && <JsonlToolResult content={task.result} isError={task.status === 'failed'} initiallyExpanded />}
        {task.fields.length > 0 && <dl>{task.fields.map(([key, value]) => <div key={key}><dt>{key}</dt><dd>{value}</dd></div>)}</dl>}
      </div>}
    </div>
  )
}
