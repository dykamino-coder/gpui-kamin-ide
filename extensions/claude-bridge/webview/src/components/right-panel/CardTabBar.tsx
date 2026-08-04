import type { JSX } from 'preact'

export interface CardTab {
  id: string
  label: string
  icon: string         // FontAwesome class without 'fa-' prefix, e.g. 'fa-list-check'
  count?: number       // optional badge — same look as chat-tab session counter
  accent?: string      // top-edge color when this tab is active. Pass var(--…)
}

interface Props {
  tabs: CardTab[]
  active: string
  onSelect: (id: string) => void
}

/** Shared tab strip for right-panel cards. Mirrors the chat TabsBar look:
 *  squared rectangles with a coloured top edge on the active one, sitting
 *  flush against the card body so the active tab visually merges with the
 *  content beneath. */
export function CardTabBar({ tabs, active, onSelect }: Props): JSX.Element {
  return (
    <div
      style="
        display:flex;align-items:stretch;
        flex-shrink:0;background:var(--overlay-soft);
      "
    >
      {tabs.map(t => {
        const isActive = t.id === active
        // Per-tab accent removed — header text + icon stay neutral so the
        // tab strip reads as a uniform header. Only the count badge uses
        // a subtle theme-aware tint (orange in light theme, blue in dark)
        // via --accent-action / --accent-action-hover.
        return (
          <button
            key={t.id}
            type="button"
            onClick={() => onSelect(t.id)}
            style={`
              flex:1 1 auto;min-width:0;
              display:inline-flex;align-items:center;justify-content:center;gap:var(--space-2);
              padding:0 var(--space-3);height:28px;
              background:${isActive ? 'var(--bg-primary)' : 'var(--bg-mantle)'};
              color:${isActive ? 'var(--text-primary)' : 'var(--text-secondary)'};
              border:none;
              font-size:var(--fs-sm);font-weight:600;letter-spacing:0.02em;
              cursor:pointer;
              transition:background var(--transition-fast),color var(--transition-fast),border-color var(--transition-fast);
            `}
          >
            <i class={`fas ${t.icon}`} style={`font-size:var(--fs-10);color:${isActive ? 'var(--text-secondary)' : 'var(--text-muted)'}`} />
            <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{t.label}</span>
            {typeof t.count === 'number' && (
              <span
                style={`
                  font-size:var(--fs-xs);padding:1px 6px;border-radius:999px;font-weight:700;
                  background:${t.count > 0 && isActive
                    ? 'color-mix(in srgb, var(--accent-action) 22%, transparent)'
                    : 'transparent'};
                  color:${t.count > 0
                    ? (isActive ? 'var(--accent-action)' : 'var(--text-secondary)')
                    : 'var(--text-muted)'};
                `}
              >
                {t.count}
              </span>
            )}
          </button>
        )
      })}
    </div>
  )
}
