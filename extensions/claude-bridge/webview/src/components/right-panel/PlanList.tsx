import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { activePlan, type PlanTask } from '../../signals/plan'
import { EmptyState } from '../ui/EmptyState'
import { PanelTabs } from '../ui/PanelTabs'
import { STATUS_STYLES } from './status-styles'

const ACTIVE_STATUSES = new Set<PlanTask['status']>(['pending', 'in_progress'])

/** Full plan rendered as a mini-kanban-ish checklist. Unlike the inline
 *  TaskCreate/TaskUpdate cards inside the transcript (which show one
 *  task-at-creation per bubble), this view aggregates the whole session
 *  and keeps statuses up-to-date so the user has a bird's-eye view of
 *  what the agent promised vs what it finished. */
export function PlanList(): JSX.Element {
  const plan = activePlan.value
  const [tab, setTab] = useState<'active' | 'completed'>('active')
  if (!plan) {
    return (
      <EmptyState
        icon="fa-list-check"
        title="No active plan."
        hint={<>Claude will populate this panel when it calls <code>TaskCreate</code>.</>}
      />
    )
  }

  const activeTasks = plan.tasks.filter((t) => ACTIVE_STATUSES.has(t.status))
  const completedTasks = plan.tasks.filter((t) => !ACTIVE_STATUSES.has(t.status))
  // Land on Completed only when there's nothing active left to watch.
  const shown = tab === 'completed' ? completedTasks : activeTasks

  return (
    <div style="display:flex;flex-direction:column;height:100%">
      <PanelTabs
        active={tab}
        onChange={setTab}
        tabs={[
          { key: 'active', label: 'Active', count: activeTasks.length },
          { key: 'completed', label: 'Completed', count: completedTasks.length },
        ]}
      />
      <div style="flex:1;overflow-y:auto;padding:var(--space-2) var(--space-2) var(--space-7);display:flex;flex-direction:column;gap:var(--space-1)">
        {shown.length === 0
          ? <div style="color:var(--text-disabled);font-size:12px;text-align:center;padding:24px">{tab === 'completed' ? 'Nothing completed yet.' : 'No active tasks.'}</div>
          : shown.map((t) => <PlanRow key={t.id} task={t} />)}
      </div>
    </div>
  )
}

function PlanRow({ task }: { task: PlanTask }): JSX.Element {
  const style = STATUS_STYLES[task.status] ?? STATUS_STYLES.pending
  const label = task.status === 'in_progress' && task.activeForm ? task.activeForm : task.subject

  return (
    <div
      data-tooltip={task.description || undefined}
      style={`
        display:flex;align-items:flex-start;gap:var(--space-2);
        padding:8px 8px;border-radius:var(--radius-sm);
        background:${style.bg};
        ${style.glow ? `box-shadow:${style.glow}` : ''}
      `}
    >
      <i class={style.icon} style={`color:${style.accent};font-size:var(--fs-base);margin-top:2px;flex-shrink:0${task.status === 'in_progress' ? ';animation:pulseIcon 1.6s ease-in-out infinite' : ''}`} />
      <div style="flex:1;min-width:0">
        <div
          style={`
            font-size:var(--fs-base);color:${style.text};line-height:var(--lh-normal);
            ${style.strike ? 'text-decoration:line-through;text-decoration-color:var(--tint-green-border)' : ''}
            word-break:break-word;
          `}
        >
          {label || <span style="color:var(--text-disabled);font-style:italic">(no subject)</span>}
        </div>
        <MetadataChips metadata={task.metadata} />
      </div>
      <span style={`font-size:var(--fs-xs);color:${style.accent};font-weight:700;letter-spacing:0.04em;margin-top:4px;flex-shrink:0;text-transform:uppercase`}>
        {style.label}
      </span>
    </div>
  )
}

function MetadataChips({ metadata }: { metadata?: Record<string, unknown> }): JSX.Element | null {
  if (!metadata) return null
  const entries = Object.entries(metadata)
  if (entries.length === 0) return null
  return (
    <div style="display:flex;flex-wrap:wrap;gap:var(--space-1);margin-top:var(--space-1)">
      {entries.map(([k, v]) => {
        const display = v == null
          ? 'null'
          : typeof v === 'string'
            ? v
            : typeof v === 'number' || typeof v === 'boolean'
              ? String(v)
              : JSON.stringify(v)
        const truncated = display.length > 48 ? display.slice(0, 46) + '…' : display
        return (
          <span
            key={k}
            data-tooltip={typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean' ? undefined : display}
            style="font-size:var(--fs-xs);padding:1px 6px;border-radius:999px;background:var(--tint-primary-medium);color:var(--accent-primary);font-family:var(--font-mono);letter-spacing:0.02em;line-height:var(--lh-base);max-width:100%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap"
          >
            <span style="color:var(--text-muted)">{k}:</span> {truncated}
          </span>
        )
      })}
    </div>
  )
}
