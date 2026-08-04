import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import { BridgeIcon } from '../icons/BridgeIcon'
import { TitlebarButton } from './TitlebarButton'
import { TitlebarQuickActions } from './TitlebarQuickActions'
import { ThemeQuickToggle } from './ThemeQuickToggle'
import { TabsBar } from './TabsBar'
import { TitlebarVersion } from './TitlebarVersion'
import { HeaderViewToggles } from '../chat-header/HeaderViewToggles'
import { sidebarWidth, sidebarVisible, sidebarMode } from '../../signals/ui'
import styles from './Titlebar.module.css'

function MicStatus(): JSX.Element {
  const bridge = useBridge()
  const [connected, setConnected] = useState(false)

  useEffect(() => {
    const check = () => {
      ;bridge.micConnected?.().then((c: boolean) => setConnected(c)).catch(() => {})
    }
    check()
    const interval = setInterval(check, 3000)
    return () => clearInterval(interval)
  }, [])

  function handleClick(): void {
    if (!connected) {
      ;bridge.micLaunchChrome?.()
    } else {
      ;bridge.micToggleVisible?.()
    }
  }

  return (
    <span
      style={`margin-left:8px;font-size:10px;color:${connected ? 'var(--accent-green)' : 'var(--text-disabled)'};display:flex;align-items:center;gap:4px;-webkit-app-region:no-drag;cursor:pointer`}
      data-tooltip={connected ? 'Click to show/hide mic window' : 'Click to start mic bridge'}
      onClick={handleClick}
    >
      <i class="fas fa-microphone" style="font-size:9px;display:inline-flex;align-items:center;justify-content:center;width:14px;height:14px;line-height:1" />
      <span style={`width:5px;height:5px;border-radius:50%;background:${connected ? 'var(--accent-green)' : 'var(--bg-overlay)'}`} />
    </span>
  )
}

export function Titlebar(): JSX.Element {
  const bridge = useBridge()
  const [isMaximized, setIsMaximized] = useState(false)

  useEffect(() => {
    return bridge.onWindowMaximized((maximized: boolean) => {
      setIsMaximized(maximized)
    })
  }, [bridge])

  // Previously rendered an ActivityIndicator showing the active tab's current
  // working title. Removed because (a) with multiple tabs open the text was
  // ambiguous — you couldn't tell which session it belonged to — and
  // (b) it was a `no-drag` zone in the middle of the titlebar, making the
  // window hard to grab. Activity status lives inside each tab's chat view.
  return (
    <div class={styles.titlebar} id="custom-titlebar">
      {/* Left cluster (icon + title) is sized to exactly the sidebar's
          current width so the tab strip starts where the chat column
          starts below. Anything inside that overflows the sidebar width
          gets ellipsised, and a longer sidebar simply pushes the tabs
          right in lockstep with the layout below. */}
      <div
        class={styles.leftCluster}
        style={(sidebarVisible.value || sidebarMode.value === 'customize') ? `width:${sidebarWidth.value}px` : 'width:auto'}
      >
        <div class={styles.icon}>
          <BridgeIcon size={20} color="var(--text-primary)" />
        </div>
        <TitlebarQuickActions />
      </div>
      {/* Tab strip + view toggles moved here from the per-chat header so
          the chat panel can stay clean. TabsBar self-hides in customize
          mode (returns null) and HeaderViewToggles mirrors that. */}
      <div class={styles.tabsSlot}><TabsBar /></div>
      <HeaderViewToggles />
      <MicStatus />
      <ThemeQuickToggle />
      <div class={styles.controls}>
        <TitlebarButton
          icon="fas fa-bug"
          variant="devtools"
          label="DevTools"
          onClick={() => bridge.windowToggleDevTools()}
        />
        <TitlebarVersion />
        <TitlebarButton
          icon="fas fa-minus"
          label="Minimize"
          onClick={() => bridge.windowMinimize()}
        />
        <TitlebarButton
          icon={isMaximized ? 'far fa-clone' : 'far fa-square'}
          label="Maximize"
          onClick={() => bridge.windowMaximize()}
        />
        <TitlebarButton
          icon="fas fa-xmark"
          variant="close"
          label="Close"
          onClick={() => bridge.windowClose()}
        />
      </div>
    </div>
  )
}
