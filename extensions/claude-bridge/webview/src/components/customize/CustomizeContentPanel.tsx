import type { JSX } from 'preact'
import { activeCustomizePanel, type CustomizePanel } from '../../signals/ui'
import { SECTIONS } from './sections'
import styles from './CustomizeContentPanel.module.css'

/** `/claudeBridgeCzSettings` → "settings" — the customize section is encoded
 *  in this iframe's URL (each section is its own webview view). */
export function sectionFromPageUrl(): CustomizePanel | null {
  const suffix = window.location.pathname.replace(/^\//, '').replace(/^claudeBridgeCz/, '')
  return suffix ? (suffix.toLowerCase() as CustomizePanel) : null
}

export function CustomizeContentPanel(): JSX.Element | null {
  // In the VSIX each section is its own always-alive page; if anything nulls
  // the signal (legacy Electron navigation paths), fall back to the section
  // encoded in this iframe's URL instead of rendering a blank page forever.
  const panel = activeCustomizePanel.value ?? sectionFromPageUrl()

  if (!panel || panel === 'landing') return null

  const section = SECTIONS.find(s => s.panel === panel)
  if (!section) return null
  const Body = section.Component

  return (
    <div class={styles.container}>
      <Body />
    </div>
  )
}
