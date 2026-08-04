// Quick-flip theme button for the titlebar. Single click cycles
// dark → light → system (and back), and the icon mirrors the resolved
// effective theme so the user sees what's actually active right now.
//
// Hotkey: Ctrl/Cmd+Shift+T cycles too — registered globally in App.tsx.

import type { JSX } from 'preact'
import { themeChoice, resolvedTheme, setTheme } from '../../theme/apply-theme'
import type { ThemeChoice } from '../../theme/apply-theme'

const NEXT: Record<ThemeChoice, ThemeChoice> = {
  dark: 'light',
  light: 'system',
  system: 'dark',
}

export function ThemeQuickToggle(): JSX.Element {
  const choice = themeChoice.value
  const eff = resolvedTheme.value
  // System-following gets a half-stroke to signal it's auto.
  const icon = choice === 'system'
    ? 'fa-circle-half-stroke'
    : eff === 'light' ? 'fa-sun' : 'fa-moon'
  const tooltip = `Theme: ${choice}${choice === 'system' ? ` (${eff})` : ''}\nClick to switch · Ctrl+Shift+T`

  return (
    <span
      style="margin-left:8px;font-size:11px;color:var(--text-muted);display:flex;align-items:center;-webkit-app-region:no-drag;cursor:pointer;padding:2px 6px;border-radius:var(--radius-xs)"
      data-tooltip={tooltip}
      onClick={() => setTheme(NEXT[choice])}
      onMouseEnter={(e: any) => { e.currentTarget.style.color = 'var(--text-primary)' }}
      onMouseLeave={(e: any) => { e.currentTarget.style.color = 'var(--text-muted)' }}
    >
      <i class={`fas ${icon}`} style="font-size:11px;display:inline-flex;align-items:center;justify-content:center;width:14px;height:14px;line-height:1" />
    </span>
  )
}
