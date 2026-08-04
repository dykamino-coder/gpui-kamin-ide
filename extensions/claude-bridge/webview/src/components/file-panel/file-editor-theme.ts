// CodeMirror 6 theme bound to our app's CSS variables — both the
// chrome (gutter, selection, cursor) and the syntax tags pull from
// existing tokens so the editor flips with the active light/dark
// theme without re-init. Reusing `--accent-primary`, `--text-*`,
// `--bg-mantle/-base/-surface`, `--accent-{red,green,yellow,…}` keeps
// the editor visually coherent with the rest of the FilePanel.

import { EditorView } from '@codemirror/view'
import type { Extension } from '@codemirror/state'
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'

function v(name: string, fallback: string): string {
  try {
    const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
    return raw || fallback
  } catch { return fallback }
}

export function buildEditorTheme(): Extension {
  // Resolve at call time so a `resolvedTheme.subscribe` re-render gets
  // the live values; callers should re-run this and replace the
  // editor's theme compartment.
  const fg = v('--text-primary', '#e6e6e6')
  const fgMuted = v('--text-muted', '#7a7d8e')
  const fgSecondary = v('--text-secondary', '#bcc0d2')
  const bgMantle = v('--bg-mantle', '#11111b')
  const bgBase = v('--bg-base', '#181826')
  const bgSurface = v('--bg-surface', '#252535')
  const accent = v('--accent-primary', '#89b4fa')
  const red = v('--accent-red', '#f38ba8')
  const green = v('--accent-green', '#a6e3a1')
  const yellow = v('--accent-yellow', '#f9e2af')
  const blue = v('--accent-primary', '#89b4fa')
  const purple = v('--accent-purple', '#cba6f7')
  const cyan = v('--accent-sapphire', '#74c7ec')
  const orange = v('--accent-orange', '#fab387')

  const chrome = EditorView.theme({
    '&': {
      color: fg,
      backgroundColor: 'transparent',
      height: '100%',
      fontSize: '12px',
      fontFamily: "ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace",
    },
    '.cm-scroller': {
      fontFamily: 'inherit',
      lineHeight: '1.5',
    },
    '.cm-content': {
      caretColor: accent,
    },
    '.cm-cursor, .cm-dropCursor': {
      borderLeftColor: accent,
      borderLeftWidth: '2px',
    },
    '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection': {
      backgroundColor: `color-mix(in srgb, ${accent} 22%, transparent)`,
    },
    '.cm-gutters': {
      backgroundColor: 'transparent',
      color: fgMuted,
      border: 'none',
    },
    '.cm-activeLine': {
      backgroundColor: `color-mix(in srgb, ${accent} 5%, transparent)`,
    },
    '.cm-activeLineGutter': {
      backgroundColor: `color-mix(in srgb, ${accent} 8%, transparent)`,
      color: fgSecondary,
    },
    '.cm-selectionMatch': {
      backgroundColor: `color-mix(in srgb, ${yellow} 20%, transparent)`,
    },
    '.cm-searchMatch': {
      backgroundColor: `color-mix(in srgb, ${yellow} 28%, transparent)`,
      outline: `1px solid ${yellow}`,
    },
    '.cm-searchMatch.cm-searchMatch-selected': {
      backgroundColor: `color-mix(in srgb, ${orange} 38%, transparent)`,
    },
    '.cm-tooltip': {
      background: bgBase,
      border: `1px solid ${bgSurface}`,
      borderRadius: '4px',
      color: fg,
    },
    '.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': {
      background: `color-mix(in srgb, ${accent} 18%, transparent)`,
      color: accent,
    },
    '.cm-panels': {
      backgroundColor: bgBase,
      color: fg,
    },
    '.cm-panels.cm-panels-bottom': { borderTop: `1px solid ${bgSurface}` },
    '.cm-panels.cm-panels-top': { borderBottom: `1px solid ${bgSurface}` },

    // Search panel (Ctrl+F / Ctrl+H) — VSCode-style. CSS Grid layout
    // forces the two rows reliably regardless of whether CM emits a
    // <br> between Find and Replace lines: row 1 = find input + nav
    // buttons + flag pills; row 2 = replace input + replace buttons.
    // Icons use Unicode glyphs (no FontAwesome dependency); native
    // button text is hidden only on icon-only buttons via font-size:0.
    '.cm-panel.cm-search': {
      position: 'relative',
      display: 'grid',
      gridTemplateColumns: 'minmax(180px, 1fr) auto auto auto auto auto auto',
      gridAutoRows: 'auto',
      columnGap: '6px',
      rowGap: '8px',
      padding: '10px 36px 10px 12px',
      margin: '8px',
      background: bgMantle,
      border: `1px solid ${bgSurface}`,
      borderRadius: '8px',
      boxShadow: '0 6px 18px rgba(0,0,0,0.18)',
      fontFamily: "ui-sans-serif, system-ui, -apple-system, 'Segoe UI', sans-serif",
      fontSize: '11px',
      borderTop: `1px solid ${bgSurface}`,
      alignItems: 'center',
    },
    // Hide CM's own <br> separator — grid handles row breaks.
    '.cm-panel.cm-search > br': { display: 'none' },

    // ─── Row 1: Find ────────────────────────────────────
    '.cm-panel.cm-search > input[name="search"]': { gridRow: '1', gridColumn: '1' },
    '.cm-panel.cm-search > button[name="next"]': { gridRow: '1', gridColumn: '2' },
    '.cm-panel.cm-search > button[name="prev"]': { gridRow: '1', gridColumn: '3' },
    '.cm-panel.cm-search > button[name="select"]': { gridRow: '1', gridColumn: '4' },
    '.cm-panel.cm-search > label:has(input[name="case"])': { gridRow: '1', gridColumn: '5' },
    '.cm-panel.cm-search > label:has(input[name="re"])': { gridRow: '1', gridColumn: '6' },
    '.cm-panel.cm-search > label:has(input[name="word"])': { gridRow: '1', gridColumn: '7' },

    // ─── Row 2: Replace ─────────────────────────────────
    // Replace input spans columns 1..4 so it lines up with the Find
    // input + nav-buttons block above; replace / replace-all sit in
    // the right cluster.
    '.cm-panel.cm-search > input[name="replace"]': { gridRow: '2', gridColumn: '1 / 5' },
    '.cm-panel.cm-search > button[name="replace"]': { gridRow: '2', gridColumn: '5 / 7' },
    '.cm-panel.cm-search > button[name="replaceAll"]': { gridRow: '2', gridColumn: '7' },

    // ─── Inputs ─────────────────────────────────────────
    '.cm-panel.cm-search input.cm-textfield, .cm-panel input[type="text"]': {
      width: '100%',
      background: bgBase,
      border: `1px solid ${bgSurface}`,
      borderRadius: '6px',
      color: fg,
      fontSize: '12px',
      fontFamily: "ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace",
      padding: '5px 10px',
      outline: 'none',
      boxSizing: 'border-box',
      transition: 'border-color 120ms, box-shadow 120ms',
    },
    '.cm-panel.cm-search input.cm-textfield:focus, .cm-panel input[type="text"]:focus': {
      borderColor: accent,
      boxShadow: `0 0 0 2px color-mix(in srgb, ${accent} 22%, transparent)`,
    },

    // ─── Buttons (base) ────────────────────────────────
    '.cm-panel.cm-search button': {
      height: '28px',
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      background: 'transparent',
      border: `1px solid ${bgSurface}`,
      borderRadius: '6px',
      color: fgSecondary,
      fontSize: '11px',
      fontFamily: 'inherit',
      fontWeight: '500',
      padding: '0 12px',
      cursor: 'pointer',
      transition: 'background 120ms, color 120ms, border-color 120ms',
    },
    '.cm-panel.cm-search button:hover': {
      background: `color-mix(in srgb, ${accent} 14%, transparent)`,
      color: accent,
      borderColor: `color-mix(in srgb, ${accent} 40%, ${bgSurface})`,
    },
    '.cm-panel.cm-search button:active': {
      background: `color-mix(in srgb, ${accent} 26%, transparent)`,
    },

    // ─── Icon-only nav buttons (hide native text, render glyph) ──
    '.cm-panel.cm-search button[name="next"], .cm-panel.cm-search button[name="prev"], .cm-panel.cm-search button[name="select"]': {
      width: '28px',
      padding: '0',
      fontSize: '0',
    },
    '.cm-panel.cm-search button[name="next"]::before, .cm-panel.cm-search button[name="prev"]::before, .cm-panel.cm-search button[name="select"]::before': {
      fontFamily: "ui-sans-serif, system-ui, sans-serif",
      fontSize: '14px',
      fontWeight: '600',
      lineHeight: '1',
    },
    '.cm-panel.cm-search button[name="next"]::before': { content: '"↓"' },
    '.cm-panel.cm-search button[name="prev"]::before': { content: '"↑"' },
    '.cm-panel.cm-search button[name="select"]::before': { content: '"≡"' },

    // ─── Flag toggles (case / regex / word) ──────────
    // <label><checkbox> match case</label> → 28x28 pill with custom
    // glyph via ::after; native checkbox + native text hidden.
    '.cm-panel.cm-search label': {
      position: 'relative',
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: '28px',
      height: '28px',
      borderRadius: '6px',
      border: `1px solid ${bgSurface}`,
      color: fgMuted,
      fontFamily: "ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace",
      fontSize: '0',
      cursor: 'pointer',
      whiteSpace: 'nowrap',
      transition: 'background 120ms, color 120ms, border-color 120ms',
    },
    '.cm-panel.cm-search label:hover': {
      background: `color-mix(in srgb, ${accent} 14%, transparent)`,
      color: accent,
      borderColor: `color-mix(in srgb, ${accent} 40%, ${bgSurface})`,
    },
    '.cm-panel.cm-search label:has(input:checked)': {
      background: `color-mix(in srgb, ${accent} 22%, transparent)`,
      color: accent,
      borderColor: `color-mix(in srgb, ${accent} 60%, ${bgSurface})`,
    },
    '.cm-panel.cm-search label input[type="checkbox"]': {
      position: 'absolute',
      opacity: '0',
      pointerEvents: 'none',
      width: '0',
      height: '0',
      margin: '0',
    },
    '.cm-panel.cm-search label::after': {
      fontSize: '11px',
      fontWeight: '600',
      fontFamily: "ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace",
    },
    '.cm-panel.cm-search label:has(input[name="case"])::after': { content: '"Aa"' },
    '.cm-panel.cm-search label:has(input[name="re"])::after': { content: '".*"' },
    '.cm-panel.cm-search label:has(input[name="word"])::after': {
      content: '"ab"',
      borderBottom: `1px solid currentColor`,
      lineHeight: '1.1',
    },

    // ─── Close ─────────────────────────────────────
    '.cm-panel.cm-search [name="close"], .cm-panel button[name="close"]': {
      position: 'absolute',
      top: '6px',
      right: '8px',
      width: '22px',
      height: '22px',
      padding: '0',
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      border: 'none',
      background: 'transparent',
      color: fgMuted,
      fontSize: '14px',
      lineHeight: '1',
      borderRadius: '4px',
    },
    '.cm-panel.cm-search [name="close"]:hover': {
      background: `color-mix(in srgb, ${accent} 14%, transparent)`,
      color: accent,
    },
  }, { dark: v('--bg-base', '#181826') !== '#fbf8f1' })
  void bgMantle // reserved — could colour matchHighlightBg if needed

  const highlight = HighlightStyle.define([
    { tag: [t.keyword, t.controlKeyword, t.modifier], color: purple },
    { tag: [t.operator, t.operatorKeyword], color: cyan },
    { tag: [t.atom, t.bool, t.null, t.number, t.special(t.string)], color: orange },
    { tag: [t.string, t.special(t.brace)], color: green },
    { tag: [t.regexp, t.escape], color: yellow },
    { tag: [t.variableName, t.propertyName], color: fg },
    { tag: [t.function(t.variableName), t.function(t.propertyName)], color: blue },
    { tag: [t.definition(t.variableName), t.definition(t.propertyName)], color: blue },
    { tag: [t.typeName, t.className, t.namespace], color: yellow },
    { tag: [t.tagName], color: red },
    { tag: [t.attributeName], color: orange },
    { tag: [t.attributeValue], color: green },
    { tag: [t.comment, t.lineComment, t.blockComment], color: fgMuted, fontStyle: 'italic' },
    { tag: [t.meta, t.processingInstruction], color: fgSecondary },
    { tag: [t.heading], color: blue, fontWeight: 'bold' },
    { tag: [t.link, t.url], color: cyan, textDecoration: 'underline' },
    { tag: [t.emphasis], fontStyle: 'italic' },
    { tag: [t.strong], fontWeight: 'bold' },
    { tag: [t.invalid], color: red, textDecoration: 'underline wavy' },
  ])

  return [chrome, syntaxHighlighting(highlight)]
}
