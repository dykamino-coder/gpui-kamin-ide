import type { JSX, ComponentChildren } from 'preact'
import { rightPanelVisible, filePanelVisible } from '../../signals/ui'
import styles from './MainPanel.module.css'

interface MainPanelProps {
  header: ComponentChildren
  content: ComponentChildren
  inputBar: ComponentChildren
  agentTiles?: ComponentChildren
  /** Absolutely-positioned layer covering the whole panel (subagent fullscreen).
   *  Rendered last so it sits above header/content/input; the panel is already
   *  position:relative + overflow:hidden, so `inset:0` clips to it. */
  overlay?: ComponentChildren
}

export function MainPanel({ header, content, inputBar, agentTiles, overlay }: MainPanelProps): JSX.Element {
  // Tighten right margin from 20px → 10px when ANY side panel sits to
  // the right of the chat (file column or activity column) — otherwise
  // MainPanel's 20px margin-right + the panel's own outer 20px margin
  // give 40px of dead space. With both panels hidden the chat keeps the
  // original 20px so it doesn't hug the window edge.
  const rightOpen = rightPanelVisible.value || filePanelVisible.value
  return (
    <div class={styles.mainPanel} style={rightOpen ? 'margin-right:10px' : undefined}>
      {header}
      {content}
      {inputBar}
      {agentTiles}
      {overlay}
    </div>
  )
}
