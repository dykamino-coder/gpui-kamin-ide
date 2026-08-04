import type { JSX } from 'preact'

interface Props {
  icon: string          // FontAwesome class without 'fa-' prefix, e.g. 'fa-list-check'
  title: string         // first line, e.g. "No active plan."
  hint?: string | JSX.Element // second line, smaller + dimmer
}

/** Standard centered empty-state placeholder used across right-panel cards
 *  and similar list views. Mirrors the visual rhythm of the original
 *  inline blocks in PlanList / TodoList / FilesList. */
export function EmptyState({ icon, title, hint }: Props): JSX.Element {
  return (
    <div style="display:flex;padding:18px 14px;color:var(--text-muted);font-size:var(--fs-base);text-align:center;line-height:var(--lh-base);flex-direction:column;align-items:center">
      <i class={`fas ${icon}`} style="font-size:var(--fs-2xl);opacity:0.4;display:block;margin-bottom:var(--space-2)" />
      {title}
      {hint && (
        <>
          <br />
          <span style="color:var(--text-disabled);font-size:var(--fs-sm)">{hint}</span>
        </>
      )}
    </div>
  )
}
