import type { JSX } from 'preact'

interface Todo {
  content?: string
  activeForm?: string
  status?: 'pending' | 'in_progress' | 'completed'
}

interface TodoWriteInput {
  todos?: Todo[]
}

/** TodoWrite: render the agent's plan as a checklist with status icons.
 *  Mirrors CLI's TodoWriteTool rendering — far more scannable than raw
 *  JSON. Completed items are strikethrough + muted; in_progress ones get
 *  the agent accent so the user can see which item is active right now. */
export function TodoWriteRender({ input }: { input: TodoWriteInput }): JSX.Element {
  const todos = Array.isArray(input.todos) ? input.todos : []
  const stats = {
    done: todos.filter(t => t.status === 'completed').length,
    active: todos.filter(t => t.status === 'in_progress').length,
    total: todos.length,
  }

  return (
    <div style="padding:6px 0">
      <div style="font-size:11px;color:var(--text-muted);padding:2px 2px 6px">
        <i class="fas fa-list-check" style="margin-right:6px;color:var(--accent-purple)" />
        Plan · {stats.done}/{stats.total} done{stats.active > 0 ? ` · ${stats.active} in progress` : ''}
      </div>
      <div style="display:flex;flex-direction:column;gap:3px">
        {todos.map((t, i) => <TodoRow key={i} todo={t} />)}
      </div>
    </div>
  )
}

function TodoRow({ todo }: { todo: Todo }): JSX.Element {
  const status = todo.status ?? 'pending'
  const label = status === 'in_progress' && todo.activeForm ? todo.activeForm : (todo.content ?? '')
  const { icon, iconColor, textColor, strike } = ({
    pending: { icon: 'fa-regular fa-square', iconColor: 'var(--text-muted)', textColor: 'var(--text-subtext)', strike: false },
    in_progress: { icon: 'fas fa-circle-half-stroke', iconColor: 'var(--accent-yellow)', textColor: 'var(--text-primary)', strike: false },
    completed: { icon: 'fa-regular fa-square-check', iconColor: 'var(--accent-green)', textColor: 'var(--text-muted)', strike: true },
  } as const)[status]

  return (
    <div style="display:flex;align-items:flex-start;gap:8px;padding:4px 6px;border-radius:var(--radius-xs);background:var(--tint-surface-soft)">
      <i class={icon} style={`color:${iconColor};font-size:13px;margin-top:2px;flex-shrink:0`} />
      <span style={`font-size:12px;color:${textColor};line-height:1.45;${strike ? 'text-decoration:line-through;text-decoration-color:var(--tint-green-border)' : ''}`}>
        {label || <span style="color:var(--text-disabled);font-style:italic">(no description)</span>}
      </span>
    </div>
  )
}
