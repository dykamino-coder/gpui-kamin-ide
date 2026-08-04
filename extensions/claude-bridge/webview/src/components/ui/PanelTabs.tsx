import type { JSX } from 'preact'

export interface PanelTab<T extends string> {
  key: T
  label: string
  count?: number
}

/** A compact 2–3 tab strip for a side panel (Plan, Agents): Active / Completed.
 *  Purely presentational — the parent owns the selected key and filters its own
 *  content. */
export function PanelTabs<T extends string>({ tabs, active, onChange }: {
  tabs: PanelTab<T>[]
  active: T
  onChange: (key: T) => void
}): JSX.Element {
  return (
    <div role="tablist" style="display:flex;gap:4px;padding:6px 8px;flex-shrink:0;border-bottom:1px solid var(--divider-soft)">
      {tabs.map((t) => {
        const on = active === t.key
        return (
          <button
            key={t.key}
            type="button"
            role="tab"
            aria-selected={on}
            onClick={() => { onChange(t.key) }}
            style={`display:inline-flex;align-items:center;gap:6px;padding:4px 10px;border:none;border-radius:var(--radius-md);cursor:pointer;font-size:11px;font-weight:600;letter-spacing:0.02em;`
              + `background:${on ? 'color-mix(in srgb, var(--accent-primary) 14%, transparent)' : 'transparent'};`
              + `color:${on ? 'var(--accent-primary)' : 'var(--text-secondary)'}`}
          >
            <span>{t.label}</span>
            {t.count !== undefined && (
              <span style={`font-size:9px;font-weight:700;padding:0 5px;border-radius:999px;`
                + `background:${on ? 'color-mix(in srgb, var(--accent-primary) 22%, transparent)' : 'var(--tint-muted-medium)'};`
                + `color:${on ? 'var(--accent-primary)' : 'var(--text-muted)'}`}>{t.count}</span>
            )}
          </button>
        )
      })}
    </div>
  )
}
