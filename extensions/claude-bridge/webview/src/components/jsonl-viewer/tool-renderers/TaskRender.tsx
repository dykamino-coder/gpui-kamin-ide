import type { JSX } from 'preact'

/** CLI's newer per-task API (TaskCreate / TaskUpdate / TaskList / etc.
 *  in src/tools/TaskCreateTool) emits ONE call per task, unlike the
 *  batched TodoWrite. Each TaskCreate looks like:
 *
 *    { subject, description, activeForm }
 *
 *  and TaskUpdate like:
 *
 *    { taskId, status }
 *
 *  We render each as a compact "to-do card" so the user sees the agent's
 *  plan items unfolding one by one, instead of a dump of raw JSON. */

export function TaskCreateRender({ input }: { input: { subject?: string; description?: string; activeForm?: string } }): JSX.Element {
  return (
    <div style="padding:4px 0">
      <div style="display:flex;align-items:center;gap:10px;padding:8px 12px;background:var(--tint-purple-soft);border:1px solid var(--tint-purple-border);border-radius:var(--radius-xs)">
        <i class="fa-regular fa-square" style="color:var(--accent-purple);font-size: var(--fs-md);flex-shrink:0" />
        <div style="flex:1;min-width:0">
          <div style="font-size:12px;color:var(--text-primary);font-weight:600;line-height:1.3">
            {input.subject || <span style="color:var(--text-disabled);font-style:italic">(no subject)</span>}
          </div>
          {input.description && (
            <div style="font-size:11px;color:var(--text-secondary);line-height:1.4;margin-top:4px">
              {input.description}
            </div>
          )}
          {input.activeForm && input.activeForm !== input.subject && (
            <div style="font-size:10px;color:var(--text-muted);font-style:italic;margin-top:4px">
              → {input.activeForm}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export function TaskUpdateRender({ input }: { input: { taskId?: string; status?: string } }): JSX.Element {
  const status = input.status ?? 'pending'
  const { icon, color, label } = ({
    pending: { icon: 'fa-regular fa-square', color: 'var(--text-muted)', label: 'pending' },
    in_progress: { icon: 'fas fa-circle-half-stroke', color: 'var(--accent-yellow)', label: 'in progress' },
    completed: { icon: 'fa-regular fa-square-check', color: 'var(--accent-green)', label: 'completed' },
    cancelled: { icon: 'fas fa-ban', color: 'var(--accent-red)', label: 'cancelled' },
    failed: { icon: 'fas fa-triangle-exclamation', color: 'var(--accent-red)', label: 'failed' },
    deleted: { icon: 'fas fa-trash', color: 'var(--text-muted)', label: 'deleted' },
  } as Record<string, { icon: string; color: string; label: string }>)[status] ?? { icon: 'fas fa-arrow-right', color: 'var(--accent-primary)', label: status }

  // Tint the card off the status colour. color-mix (unlike the old hex-parse
  // helper) works with the CSS-var colours we actually pass — the previous code
  // matched only literal #rrggbb, so a var fell through and painted the card at
  // FULL saturation, drowning the text. Keep the fill subtle so the neutral
  // label text stays legible; the icon + status word carry the colour.
  const fill = `color-mix(in srgb, ${color} 12%, transparent)`
  const stroke = `color-mix(in srgb, ${color} 34%, transparent)`
  return (
    <div style="padding:4px 0">
      <div style={`display:flex;align-items:center;gap:10px;padding:6px 12px;background:${fill};border:1px solid ${stroke};border-radius:var(--radius-xs)`}>
        <i class={icon} style={`color:${color};font-size:13px;flex-shrink:0`} />
        <div style="flex:1;min-width:0;font-size:12px;color:var(--text-secondary)">
          Task <span style="font-family:var(--font-mono);color:var(--text-primary);font-weight:600">#{input.taskId ?? '?'}</span>
          <span style="margin:0 6px;color:var(--text-muted)">→</span>
          <span style={`color:${color};font-weight:600;text-transform:uppercase;letter-spacing:0.03em;font-size:10px`}>{label}</span>
        </div>
      </div>
    </div>
  )
}
