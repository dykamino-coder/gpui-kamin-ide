// Three-way theme picker — Dark / Light / System. Symmetric segmented
// control. Uses the theme application + persistence helpers from
// `theme/apply-theme.ts` so the choice survives reloads.

import type { JSX } from 'preact'
import { themeChoice, setTheme } from '../../../theme/apply-theme'
import type { ThemeChoice } from '../../../theme/apply-theme'

const OPTIONS: Array<{ value: ThemeChoice; label: string; icon: string }> = [
  { value: 'dark', label: 'Dark', icon: 'fa-moon' },
  { value: 'light', label: 'Light', icon: 'fa-sun' },
  { value: 'system', label: 'System', icon: 'fa-circle-half-stroke' },
]

export function ThemeToggle(): JSX.Element {
  const current = themeChoice.value
  return (
    <div style="display:flex;gap:6px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);padding:3px">
      {OPTIONS.map(opt => {
        const active = opt.value === current
        return (
          <button
            key={opt.value}
            type="button"
            onClick={() => setTheme(opt.value)}
            aria-pressed={active}
            style={`
              flex:1;display:inline-flex;align-items:center;justify-content:center;gap:6px;
              padding:6px 12px;border-radius:var(--radius-xs);border:none;cursor:pointer;
              background:${active ? 'var(--bg-surface)' : 'transparent'};
              color:${active ? 'var(--text-primary)' : 'var(--text-muted)'};
              font-size:12px;font-weight:${active ? 600 : 500};
              transition:background var(--transition-fast),color var(--transition-fast);
            `}
          >
            <i class={`fas ${opt.icon}`} style="font-size:11px" />
            {opt.label}
          </button>
        )
      })}
    </div>
  )
}
