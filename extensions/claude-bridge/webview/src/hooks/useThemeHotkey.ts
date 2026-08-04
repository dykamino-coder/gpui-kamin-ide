// Ctrl/Cmd+Shift+T cycles theme: dark → light → system → dark.
// Cheap to install — single document keydown listener gated on the
// modifier combo. Skips when focus is inside an input/textarea so the
// shortcut doesn't intercept typing.

import { useEffect } from 'preact/hooks'
import { themeChoice, setTheme } from '../theme/apply-theme'
import type { ThemeChoice } from '../theme/apply-theme'

const NEXT: Record<ThemeChoice, ThemeChoice> = {
  dark: 'light',
  light: 'system',
  system: 'dark',
}

export function useThemeHotkey(): void {
  useEffect(() => {
    function onKey(e: KeyboardEvent): void {
      if (!e.shiftKey) return
      if (!e.ctrlKey && !e.metaKey) return
      if (e.key !== 'T' && e.key !== 't') return
      const tag = (document.activeElement as HTMLElement | null)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      e.preventDefault()
      setTheme(NEXT[themeChoice.value])
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [])
}
