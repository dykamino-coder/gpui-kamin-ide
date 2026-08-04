import type { JSX, ComponentChildren } from 'preact'
import { Titlebar } from '../titlebar/Titlebar'
import { FilePanel } from '../file-panel/FilePanel'
import { RightPanel } from '../right-panel/RightPanel'
import { sidebarVisible, sidebarMode } from '../../signals/ui'
import styles from './AppLayout.module.css'

interface AppLayoutProps {
  sidebar: ComponentChildren
  mainContent: ComponentChildren
}

export function AppLayout({ sidebar, mainContent }: AppLayoutProps): JSX.Element {
  // When the user collapses the sidebar via the titlebar hamburger,
  // we need the chat column to keep some breathing room from the
  // window edge — mirror the right-side `--space-5` margin so the
  // layout stays visually centred. With the sidebar visible the
  // sidebar itself provides that gap. Customize mode always pins the
  // sidebar visible (it's the settings nav), so the padding only
  // kicks in when the user actually has it collapsed.
  const sidebarShown = sidebarVisible.value || sidebarMode.value === 'customize'
  const bodyStyle = sidebarShown ? undefined : 'padding-left:var(--space-5)'
  return (
    <div class={styles.appWrapper} id="app-wrapper">
      <Titlebar />
      <div class={styles.body} id="app" style={bodyStyle}>
        {sidebar}
        {mainContent}
        <FilePanel />
        <RightPanel />
      </div>
    </div>
  )
}
