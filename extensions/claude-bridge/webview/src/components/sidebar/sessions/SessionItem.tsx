import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useComputed } from '@preact/signals'
import type { TabInfo } from '../../../../shared/types'
import { tabActivity, tabWaiting } from '../../../signals/ui'
import { activeWidgets } from '../../../signals/widgets'
import { SessionStatusDot } from './SessionStatusDot'
import { SessionLabel } from './SessionLabel'
import { SessionTime } from './SessionTime'
import { SessionRenameButton } from './SessionRenameButton'
import { SessionCloseButton } from './SessionCloseButton'
import { SessionPinButton } from './SessionPinButton'
import { togglePinTab, moveTabTo } from '../../../lib/tabs-persist'
import { getAgentColor, resolvePinnedColor } from '../../../utils/agent-color'
import { pinnedTitleColors } from '../../../signals/ui'

interface SessionItemProps {
  tab: TabInfo
  isActive: boolean
  onClick: () => void
}

const DND_MIME = 'application/x-ocb-tab-id'

export function SessionItem({ tab, isActive, onClick }: SessionItemProps): JSX.Element {
  const isUnnamed = !tab.label || /^#\d+$/.test(tab.label)
  const displayLabel = tab.sessionTitle || (isUnnamed ? 'New session' : tab.label)
  const hasName = !!(tab.sessionTitle || !isUnnamed)
  const [dragHover, setDragHover] = useState<null | 'top' | 'bottom'>(null)
  const [dragging, setDragging] = useState(false)

  const pendingCount = useComputed(() =>
    activeWidgets.value.filter(w =>
      !w.resolved &&
      (w.type === 'askUser' || w.type === 'elicitation' || w.type === 'permission') &&
      w.data?.tabId === tab.id
    ).length
  )

  function handleRename(): void {
    const bridge = (window as any).electronBridge
    bridge?.sendInput(tab.id, '/rename\r')
  }

  function handleClose(e: Event): void {
    e.stopPropagation()
    const bridge = (window as any).electronBridge
    bridge?.closeTab(tab.id)
  }

  function handlePin(): void {
    togglePinTab(tab.id)
  }

  function handleClick(e: MouseEvent): void {
    const target = e.target as HTMLElement
    if (target.closest('.session-close') || target.closest('.session-rename') || target.closest('.session-pin')) return
    onClick()
  }

  function onDragStart(e: DragEvent): void {
    if (!e.dataTransfer) return
    e.dataTransfer.setData(DND_MIME, tab.id)
    e.dataTransfer.effectAllowed = 'move'
    setDragging(true)
  }

  function onDragEnd(): void {
    setDragging(false)
    setDragHover(null)
  }

  function onDragOver(e: DragEvent): void {
    if (!e.dataTransfer) return
    // Only accept drops that carry our MIME — ignore files, text, etc.
    if (!e.dataTransfer.types.includes(DND_MIME)) return
    e.preventDefault()
    e.dataTransfer.dropEffect = 'move'
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const mid = rect.top + rect.height / 2
    setDragHover(e.clientY < mid ? 'top' : 'bottom')
  }

  function onDragLeave(): void {
    setDragHover(null)
  }

  function onDrop(e: DragEvent): void {
    if (!e.dataTransfer) return
    const srcId = e.dataTransfer.getData(DND_MIME)
    setDragHover(null)
    if (!srcId || srcId === tab.id) return
    e.preventDefault()
    moveTabTo(srcId, tab.id)
  }

  const pinned = !!tab.pinned
  const colorKey = tab.sessionTitle || (hasName ? displayLabel : '')
  const pinnedColor = resolvePinnedColor(colorKey ? pinnedTitleColors.value[colorKey] : null)
  const sessionColor = hasName ? (pinnedColor || getAgentColor(colorKey)) : null
  const classes = [
    'sidebar-session',
    isActive ? 'active' : '',
    pinned ? 'pinned' : '',
    sessionColor ? 'tinted' : '',
    dragging ? 'dnd-dragging' : '',
    dragHover === 'top' ? 'dnd-over-top' : '',
    dragHover === 'bottom' ? 'dnd-over-bottom' : '',
  ].filter(Boolean).join(' ')

  return (
    <div
      class={classes}
      data-tab-id={tab.id}
      style={sessionColor ? `--tab-color:${sessionColor}` : undefined}
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      onClick={handleClick}
    >
      {tabWaiting.value.get(tab.id)
        ? <span class="session-status" style="color:var(--accent-yellow)" title="Waiting for you"><i class="fas fa-circle-question" /></span>
        : tabActivity.value.get(tab.id)?.isWorking
          ? <span class="session-status session-spinner"><i class="fas fa-spinner fa-spin" /></span>
          : <SessionStatusDot status={tab.status} connectedColor={sessionColor} />
      }
      <SessionLabel label={displayLabel} hasName={hasName} sessionTitle={tab.sessionTitle} />
      {pendingCount.value > 0 && (
        <span
          class="session-bell"
          data-tooltip={pendingCount.value === 1 ? 'Waiting for your answer' : `${pendingCount.value} pending questions`}
          style="color:var(--accent-yellow);margin-left:4px;font-size:11px;display:inline-flex;align-items:center;gap:3px"
        >
          <i class="fas fa-bell" />
          {pendingCount.value > 1 && <span style="font-size:10px;font-weight:600">{pendingCount.value}</span>}
        </span>
      )}
      <SessionTime createdAt={tab.createdAt} />
      <SessionPinButton pinned={pinned} onClick={handlePin} />
      <SessionRenameButton onClick={handleRename} />
      <SessionCloseButton onClick={handleClose} />
    </div>
  )
}
