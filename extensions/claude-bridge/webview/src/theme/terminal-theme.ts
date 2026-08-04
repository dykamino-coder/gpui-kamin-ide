// xterm.js can't read CSS custom properties, so we resolve them at call-
// time via getComputedStyle and rebuild the theme object. Re-call on
// theme change (subscribe to resolvedTheme signal) and pass the result
// to terminal.options.theme = … for live colour updates.
//
// We use dedicated --term-* tokens (declared per theme) so the xterm
// palette can diverge from the UI accent palette where needed — most
// importantly in light mode, where ANSI text needs much deeper inks
// than chip/button accents to stay readable at 11px.

import type { ITheme } from '@xterm/xterm'

function v(name: string, fallback: string): string {
  try {
    const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
    return raw || fallback
  } catch { return fallback }
}

export function buildTerminalTheme(): ITheme {
  return {
    background: v('--term-bg', '#1d1d28'),
    foreground: v('--term-fg', '#cfd4e2'),
    cursor: v('--term-cursor', '#f5e0dc'),
    selectionBackground: v('--term-selection', '#515567'),
    selectionForeground: v('--term-fg', '#cfd4e2'),
    black: v('--term-black', '#515567'),
    red: v('--term-red', '#f38ba8'),
    green: v('--term-green', '#a6e3a1'),
    yellow: v('--term-yellow', '#f9e2af'),
    blue: v('--term-blue', '#89b4fa'),
    magenta: v('--term-magenta', '#f5c2e7'),
    cyan: v('--term-cyan', '#94e2d5'),
    white: v('--term-white', '#afb6ca'),
    brightBlack: v('--term-bright-black', '#60667b'),
    brightRed: v('--term-red', '#f38ba8'),
    brightGreen: v('--term-green', '#a6e3a1'),
    brightYellow: v('--term-yellow', '#f9e2af'),
    brightBlue: v('--term-blue', '#89b4fa'),
    brightMagenta: v('--term-magenta', '#f5c2e7'),
    brightCyan: v('--term-cyan', '#94e2d5'),
    brightWhite: v('--term-bright-white', '#adb3c7'),
  }
}

/** Backwards-compatible alias for callers that import the constant. */
export const TERMINAL_THEME = buildTerminalTheme()
