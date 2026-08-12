import type { JSX } from 'preact'
import { sidebarMode, activeCustomizePanel, sidebarVisible, setSidebarVisible } from '../../signals/ui'

/** Replaces the static "Open Claude Bridge" title with a row of icon
 *  buttons mirroring the sidebar's quick-action stack. The leftmost
 *  button toggles the sidebar itself; the rest match
 *  SidebarTopActions in `sidebar/sessions/`. Sits inside the titlebar
 *  `leftCluster` so it stays aligned with the sidebar column edge. */
export function TitlebarQuickActions(): JSX.Element {
  function leaveCustomize(): void {
    if (sidebarMode.value === 'customize') {
      sidebarMode.value = 'sessions'
      activeCustomizePanel.value = null
    }
  }
  async function newSessionNoFolder(): Promise<void> {
    const bridge = (window as { kaminBridge?: any }).kaminBridge
    if (!bridge) return
    const config = await bridge.getConfig()
    if (!config?.serverUrl || !config?.token) return
    await bridge.createTab({ serverUrl: config.serverUrl, token: config.token, cwd: '' })
    leaveCustomize()
  }
  async function newSessionWithFolder(): Promise<void> {
    const bridge = (window as { kaminBridge?: any }).kaminBridge
    if (!bridge) return
    const folder = await bridge.selectFolder?.()
    if (!folder) return
    const config = await bridge.getConfig()
    if (!config?.serverUrl || !config?.token) return
    await bridge.createTab({ serverUrl: config.serverUrl, token: config.token, cwd: folder })
    leaveCustomize()
  }
  function openCustomize(): void {
    sidebarMode.value = 'customize'
    if (!activeCustomizePanel.value || activeCustomizePanel.value === 'landing') {
      activeCustomizePanel.value = 'settings'
    }
  }
  function openStats(): void {
    sidebarMode.value = 'customize'
    activeCustomizePanel.value = 'stats'
  }

  const sbVisible = sidebarVisible.value
  // The four "create / navigate" icons duplicate buttons that already
  // sit at the top of the rendered sidebar. Hide them while the
  // sidebar is open (or pinned-open via Customize mode) so the
  // titlebar doesn't carry redundant controls.
  const showCreateActions = !(sbVisible || sidebarMode.value === 'customize')

  return (
    <div
      class="titlebar-quick-actions"
      style="display:inline-flex;align-items:center;gap:1px;padding:0 6px;-webkit-app-region:no-drag"
    >
      <ActionBtn
        icon="fa-bars"
        tooltip={sbVisible ? 'Hide sessions sidebar' : 'Show sessions sidebar'}
        active={sbVisible}
        onClick={() => setSidebarVisible(!sbVisible)}
      />
      {showCreateActions && (
        <>
          {/* Divider between toggle and create-actions when both
              clusters are showing. */}
          <span style="width:1px;height:14px;background:var(--bg-surface);margin:0 4px" />
          <ActionBtn
            icon="fa-gear"
            tooltip="Customize"
            onClick={openCustomize}
          />
          <ActionBtn
            icon="fa-square-plus"
            tooltip="New session — no folder"
            onClick={newSessionNoFolder}
          />
          <ActionBtn
            icon="fa-circle-plus"
            tooltip="New session — pick folder"
            onClick={newSessionWithFolder}
          />
          <ActionBtn
            icon="fa-chart-column"
            tooltip="Stats"
            onClick={openStats}
          />
        </>
      )}
    </div>
  )
}

function ActionBtn({ icon, tooltip, active, onClick }: {
  icon: string
  tooltip: string
  active?: boolean
  onClick: () => void
}): JSX.Element {
  return (
    <button
      type="button"
      onClick={onClick}
      data-tooltip={tooltip}
      aria-pressed={active}
      style={`
        width:28px;height:28px;display:inline-flex;align-items:center;justify-content:center;
        border:none;border-radius:var(--radius-sm);
        background:${active ? 'color-mix(in srgb, var(--accent-primary) 14%, transparent)' : 'transparent'};
        color:${active ? 'var(--accent-primary)' : 'var(--text-secondary)'};
        cursor:pointer;padding:0;
        transition:background 120ms,color 120ms;
      `}
      onMouseEnter={(e: any) => { if (!active) e.currentTarget.style.background = 'var(--bg-surface)' }}
      onMouseLeave={(e: any) => { if (!active) e.currentTarget.style.background = 'transparent' }}
    >
      <i class={`fas ${icon}`} style="font-size:12px" />
    </button>
  )
}
