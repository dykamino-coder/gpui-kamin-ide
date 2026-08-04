/**
 * Deterministic colour for a named session/agent (hash of name → palette
 * index). Each token is resolved from a CSS custom property so the
 * runtime palette tracks the active theme automatically.
 *
 * Live-theme reactivity: every export below TOUCHES `resolvedTheme.value`
 * before doing the read. When called inside a Preact JSX render or a
 * `computed`, this registers the caller as a reader of the theme signal —
 * so when the user flips dark↔light, every component that paints with
 * an agent colour re-renders automatically. Without that touch the JSX
 * stays cached and shows stale colours from the previous theme.
 */

import { resolvedTheme } from '../theme/apply-theme'

export const PALETTE_TOKENS = [
  '--accent-green',
  '--accent-yellow',
  '--accent-purple',
  '--accent-teal',
  '--accent-pink',
  '--accent-orange',
  '--accent-red',
  '--accent-sapphire',
  '--accent-blue-soft-2',  // lavender — distinct from user blue
  '--accent-maroon',
  '--text-muted-2',        // gray-blue
] as const

/** Resolve a stored pin colour. New entries are stored as CSS variable
 *  names (`--accent-pink`) so they re-resolve on theme change. Old
 *  entries are concrete hex values from the previous storage scheme —
 *  return them as-is so previously-pinned tabs keep working until the
 *  user re-picks. */
export function resolvePinnedColor(stored: string | null | undefined): string | null {
  if (!stored) return null
  if (stored.startsWith('--')) return `var(${stored})`
  return stored
}

const FALLBACKS = [
  '#a6e3a1', '#f9e2af', '#cba6f7', '#94e2d5', '#f5c2e7',
  '#fab387', '#f38ba8', '#74c7ec', '#b4befe', '#eba0ac', '#a6adc8',
]

function readToken(token: string, fallback: string): string {
  try {
    const v = getComputedStyle(document.documentElement).getPropertyValue(token).trim()
    return v || fallback
  } catch { return fallback }
}

/** Live snapshot of the agent palette. Indexed access + `.length` +
 *  `.map` + iteration all read CSS variables fresh AND touch the theme
 *  signal so consumers re-render on theme flip. */
export const AGENT_PALETTE: readonly string[] = new Proxy([] as string[], {
  get(_t, prop) {
    if (prop === 'length') return PALETTE_TOKENS.length
    // Touch the signal so any JSX using indexed access re-renders.
    void resolvedTheme.value
    if (typeof prop === 'string' && /^\d+$/.test(prop)) {
      const i = Number(prop)
      return readToken(PALETTE_TOKENS[i] ?? '', FALLBACKS[i] ?? '#888')
    }
    if (prop === Symbol.iterator) {
      return function* () {
        for (let i = 0; i < PALETTE_TOKENS.length; i++) {
          yield readToken(PALETTE_TOKENS[i]!, FALLBACKS[i]!)
        }
      }
    }
    if (prop === 'map') {
      return (fn: (v: string, i: number) => unknown) =>
        PALETTE_TOKENS.map((tok, i) => fn(readToken(tok, FALLBACKS[i]!), i))
    }
    return undefined
  },
})

function hashString(str: string): number {
  let hash = 0
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) - hash + str.charCodeAt(i)) | 0
  }
  return Math.abs(hash)
}

export function getAgentColor(name: string | undefined | null): string | null {
  if (!name || !name.trim()) return null
  // Touch the theme signal so the caller's component re-renders when
  // the theme flips. Without this the returned colour is correct only
  // at the moment of the original render.
  void resolvedTheme.value
  const idx = hashString(name) % PALETTE_TOKENS.length
  return readToken(PALETTE_TOKENS[idx]!, FALLBACKS[idx]!)
}
