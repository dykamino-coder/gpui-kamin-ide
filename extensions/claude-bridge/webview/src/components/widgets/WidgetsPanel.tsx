import { useSignalEffect } from '@preact/signals'
import { activeWidgets } from '../../signals/widgets'
import { activeTabId } from '../../signals/tabs'
import { activeLocalQueue } from '../../signals/queue'
import { ElicitationWidget } from './ElicitationWidget'
import { McpElicitationWidget, type McpElicitationData } from './McpElicitationWidget'
import { PermissionWidget } from './PermissionWidget'
import { AskUserWidget } from './AskUserWidget'
import { QueueWidget } from './QueueWidget'
import type { ElicitationRequest, PermissionRequest } from '../../../shared/types'
import styles from './WidgetsPanel.module.css'

export function WidgetsPanel() {
  const currentTabId = activeTabId.value
  // Show only widgets belonging to the active tab. Every widget now carries its
  // source tabId (permission too), so a request arriving for a non-active chat
  // stays scoped to that chat and re-appears when the user switches tabs.
  const widgets = activeWidgets.value.filter(w => {
    const widgetTabId = w.data?.tabId
    return !widgetTabId || widgetTabId === currentTabId
  })

  // Resolved widgets are removed immediately on response — no lingering disabled cards.
  // Kept as a safety net only for orphaned resolved state older than 2 min.
  useSignalEffect(() => {
    const stale = activeWidgets.value.filter(w => w.resolved).length
    if (stale > 0) {
      setTimeout(() => {
        activeWidgets.value = activeWidgets.value.filter(w => !w.resolved)
      }, 2 * 60 * 1000)
    }
  })

  const hasQueue = activeLocalQueue.value.length > 0
  const hasWidgets = widgets.length > 0 || hasQueue
  const panelClass = [styles.panel, hasWidgets ? styles.hasWidgets : ''].filter(Boolean).join(' ')

  return (
    <div class={panelClass}>
      {widgets.map(w => {
        if (w.type === 'elicitation') {
          return (
            <ElicitationWidget
              key={w.requestId}
              tabId={w.data.tabId}
              request={w.data.request as ElicitationRequest}
            />
          )
        }
        if (w.type === 'permission') {
          return (
            <PermissionWidget
              key={w.requestId}
              request={w.data.request as PermissionRequest}
            />
          )
        }
        if (w.type === 'askUser') {
          return (
            <AskUserWidget
              key={w.requestId}
              tabId={w.data.tabId}
              data={w.data.data}
            />
          )
        }
        if (w.type === 'mcpElicitation') {
          return <McpElicitationWidget key={w.requestId} data={w.data as McpElicitationData} />
        }
        return null
      })}
      <QueueWidget />
    </div>
  )
}
