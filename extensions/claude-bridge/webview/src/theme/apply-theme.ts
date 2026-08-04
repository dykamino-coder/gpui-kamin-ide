import { storage } from "@bridge/storage"
// Theme application + persistence.
//
// Persistence: localStorage key `theme` holds 'dark' | 'light' | 'system'.
// On 'system' we read `prefers-color-scheme` and stay subscribed so the
// app re-applies when the OS preference flips at runtime (without a
// reload). Writing to <html data-theme=…> drives every CSS rule scoped
// to `[data-theme="dark"]` / `[data-theme="light"]`.

import { signal } from '@preact/signals'

export type ThemeChoice = 'dark' | 'light' | 'system'
export type ResolvedTheme = 'dark' | 'light'

const LS_KEY = 'theme'

/** User-facing setting, includes 'system'. Drives the toggle UI. */
export const themeChoice = signal<ThemeChoice>(loadChoice())
/** Effective theme actually applied. Drives any JS-side colour reads
 *  (e.g. xterm palette) and tracks 'system' → resolved value. */
export const resolvedTheme = signal<ResolvedTheme>('dark')

/** Bumped on EVERY host theme push (`__kaminTheme`), including switching
 *  between two themes of the SAME kind (Kamin Dark → Bridge Dark) where
 *  `resolvedTheme` doesn't change. JS-side colour reads that bake CSS-var
 *  values into JS state — e.g. the xterm palette in `terminal.options.theme`,
 *  which doesn't auto-react to a CSS-var swap — depend on this to re-read. */
export const themeNonce = signal(0)

function loadChoice(): ThemeChoice {
  try {
    const raw = storage.getItem(LS_KEY)
    if (raw === 'dark' || raw === 'light' || raw === 'system') return raw
  } catch { /* ignore */ }
  return 'system'
}

/** KaminIDE injects `vscode-dark`/`vscode-light` onto <html> for every webview.
 *  When present it's authoritative — the Bridge follows the IDE theme. */
function hostTheme(): ResolvedTheme | null {
  try {
    const cl = document.documentElement.classList
    if (cl.contains('vscode-light')) return 'light'
    if (cl.contains('vscode-dark')) return 'dark'
  } catch { /* ignore */ }
  return null
}

function resolve(choice: ThemeChoice): ResolvedTheme {
  // The IDE-injected class wins over the stored choice so the Bridge always
  // matches KaminIDE's active theme.
  const host = hostTheme()
  if (host) return host
  if (choice !== 'system') return choice
  try {
    return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark'
  } catch { return 'dark' }
}

function apply(resolved: ResolvedTheme): void {
  const root = document.documentElement
  root.setAttribute('data-theme', resolved)
  resolvedTheme.value = resolved
  // Push to main so toast mini-windows flip live too. Bridge may not be
  // exposed yet during initial paint — guard the call.
  try { (window as any).electronBridge?.setTheme?.(resolved) } catch { /* noop */ }
}

/** Initial paint — read stored choice and apply before render so we
 *  don't flash the wrong theme. Called from main.tsx top-level. */
export function applyInitialTheme(): void {
  const choice = themeChoice.value
  apply(resolve(choice))

  // Re-resolve on OS preference change (only matters when choice === 'system'
  // AND KaminIDE didn't inject a theme class).
  try {
    const mq = window.matchMedia('(prefers-color-scheme: light)')
    const onChange = () => {
      if (themeChoice.value === 'system') apply(resolve('system'))
    }
    if (typeof mq.addEventListener === 'function') mq.addEventListener('change', onChange)
    else if (typeof (mq as any).addListener === 'function') (mq as any).addListener(onChange)
  } catch { /* ignore */ }

  // Re-apply when KaminIDE flips its theme — it toggles the vscode-dark/light
  // class on <html> (re-injected on re-serve). Mirror it live.
  try {
    const obs = new MutationObserver(() => {
      const host = hostTheme()
      if (host) apply(host)
    })
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
  } catch { /* ignore */ }

  // The host pushes `{__kaminTheme}` on ANY theme change (the injected head
  // script swaps the `--vscode-*` / `--editor-*` values). A same-kind switch
  // (Kamin Dark → Bridge Dark) doesn't move `resolvedTheme`, so bump a nonce so
  // JS-baked colour reads (xterm palette) rebuild from the new vars.
  try {
    window.addEventListener('message', (e: MessageEvent) => {
      const d = e.data as { __kaminTheme?: unknown } | null
      if (d && d.__kaminTheme) themeNonce.value++
    })
  } catch { /* ignore */ }
}

/** Set + persist the user's theme choice. */
export function setTheme(choice: ThemeChoice): void {
  themeChoice.value = choice
  try { storage.setItem(LS_KEY, choice) } catch { /* ignore */ }
  apply(resolve(choice))
}
