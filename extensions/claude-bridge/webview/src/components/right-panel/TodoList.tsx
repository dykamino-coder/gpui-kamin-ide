import type { JSX } from 'preact'
import { activeTodos, type TodoItem } from '../../signals/todos'
import { EmptyState } from '../ui/EmptyState'
import { STATUS_STYLES, type Status } from './status-styles'

/** Mirrors PlanList visual style but reflects the LAST TodoWrite call.
 *  Whole list is overwritten on each TodoWrite, so we just render the
 *  snapshot — no merge logic needed. */
export function TodoList(): JSX.Element {
  const snap = activeTodos.value
  if (!snap) {
    return (
      <EmptyState
        icon="fa-list-ul"
        title="No todos yet."
        hint={<>Claude will populate this panel when it calls <code>TodoWrite</code>.</>}
      />
    )
  }

  return (
    <div style="display:flex;flex-direction:column;height:100%">
      <header style="padding:var(--space-3) var(--space-4);border-bottom:1px solid var(--bg-surface);font-size:var(--fs-sm);color:var(--text-secondary);display:flex;align-items:center;gap:var(--space-2);flex-shrink:0">
        <i class={snap.allDone ? 'fas fa-check-double' : 'fas fa-list-ul'} style={`color:${snap.allDone ? 'var(--accent-green)' : 'var(--accent-primary)'};font-size:var(--fs-base)`} />
        <span style="font-weight:600">
          {snap.completed}/{snap.total}
        </span>
        <span style="color:var(--text-muted)">
          {snap.allDone
            ? 'all done'
            : snap.inProgress > 0
              ? `${snap.inProgress} in progress`
              : 'pending'}
        </span>
        {snap.allDone && (
          <span style="margin-left:auto;padding:1px 8px;border-radius:999px;background:var(--tint-green-strong);color:var(--accent-green);font-size:var(--fs-xs);font-weight:700;letter-spacing:0.06em">
            COMPLETE
          </span>
        )}
      </header>
      <div style="flex:1;overflow-y:auto;padding:var(--space-2) var(--space-2) var(--space-7);display:flex;flex-direction:column;gap:var(--space-1)">
        {snap.items.map((t, i) => <TodoRow key={i} item={t} />)}
      </div>
    </div>
  )
}

function TodoRow({ item }: { item: TodoItem }): JSX.Element {
  // TodoItem.status is a strict subset of Status; widening is safe.
  const style = STATUS_STYLES[item.status as Status] ?? STATUS_STYLES.pending
  const label = item.status === 'in_progress' && item.activeForm ? item.activeForm : item.content

  return (
    <div
      data-tooltip={item.activeForm && item.activeForm !== item.content ? item.content : undefined}
      style={`
        display:flex;align-items:flex-start;gap:var(--space-2);
        padding:8px 8px;border-radius:var(--radius-sm);
        background:${style.bg};
        ${style.glow ? `box-shadow:${style.glow}` : ''}
      `}
    >
      <i class={style.icon} style={`color:${style.accent};font-size:var(--fs-base);margin-top:2px;flex-shrink:0${item.status === 'in_progress' ? ';animation:pulseIcon 1.6s ease-in-out infinite' : ''}`} />
      <div style="flex:1;min-width:0">
        <div
          style={`
            font-size:var(--fs-base);color:${style.text};line-height:var(--lh-normal);
            ${style.strike ? 'text-decoration:line-through;text-decoration-color:var(--tint-green-border)' : ''}
            word-break:break-word;
          `}
        >
          {label || <span style="color:var(--text-disabled);font-style:italic">(empty)</span>}
        </div>
      </div>
      <span style={`font-size:var(--fs-xs);color:${style.accent};font-weight:700;letter-spacing:0.04em;margin-top:4px;flex-shrink:0;text-transform:uppercase`}>
        {style.label}
      </span>
    </div>
  )
}
