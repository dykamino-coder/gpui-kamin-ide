// Live highlight.js theme swapper. Imports both stylesheets via Vite's
// `?inline` so the CSS text is bundled, then injects exactly one
// <style> element whose textContent we swap when resolvedTheme flips.
// Avoids the FOUC of network-loaded <link rel="stylesheet"> in
// production (no /node_modules to fetch from after packaging).

// @ts-expect-error — Vite ?inline returns string at build time.
import darkCss from 'highlight.js/styles/atom-one-dark.css?inline'
// @ts-expect-error — Vite ?inline returns string at build time.
import lightCss from 'highlight.js/styles/atom-one-light.css?inline'
import { effect } from '@preact/signals'
import { resolvedTheme } from './apply-theme'

const STYLE_ID = 'hljs-theme-style'

function ensureStyleNode(): HTMLStyleElement {
  let el = document.getElementById(STYLE_ID) as HTMLStyleElement | null
  if (!el) {
    el = document.createElement('style')
    el.id = STYLE_ID
    document.head.appendChild(el)
  }
  return el
}

export function installHljsThemeSwapper(): void {
  const node = ensureStyleNode()
  effect(() => {
    node.textContent = resolvedTheme.value === 'light' ? (lightCss as unknown as string) : (darkCss as unknown as string)
  })
}
