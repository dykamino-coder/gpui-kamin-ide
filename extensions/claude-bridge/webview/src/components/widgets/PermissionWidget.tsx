import type { PermissionRequest } from '../../../shared/types'
import { activeWidgets } from '../../signals/widgets'
import { useBridge } from '../../hooks/useBridge'
import { WidgetHeader } from './WidgetHeader'
import { WidgetBody } from './WidgetBody'
import { WidgetActions } from './WidgetActions'
import { PermissionInputSummary } from './PermissionInputSummary'
import styles from './PermissionWidget.module.css'

const CATEGORY_LABELS: Record<string, string> = {
  'write': 'File Write',
  'shell': 'Shell Command',
  'git': 'Git Operation',
  'read-only': 'File Read',
  'network': 'Network',
  'agent': 'Agent',
  'task': 'Task',
  'team': 'Team',
  'ui': 'UI',
}

interface PermissionWidgetProps {
  request: PermissionRequest
}

export function PermissionWidget({ request }: PermissionWidgetProps) {
  const bridge = useBridge()

  const categoryLabel = CATEGORY_LABELS[request.category] || request.category

  function dismissCard() {
    activeWidgets.value = activeWidgets.value.filter(w => w.requestId !== request.requestId)
  }

  function handleAllowOnce() {
    bridge.respondPermission(request.requestId, 'allow_once')
    dismissCard()
  }

  function handleAllowSession() {
    bridge.respondPermission(request.requestId, 'allow_session')
    dismissCard()
  }

  function handleDeny() {
    bridge.respondPermission(request.requestId, 'deny')
    dismissCard()
  }

  return (
    <div class={styles.card}>
      <WidgetHeader icon="fas fa-shield-halved" title="Permission Required" typeBadge={categoryLabel} />
      <WidgetBody>
        <div class="widget-message" style="font-weight:600;margin-bottom:0;">
          {request.toolName}
        </div>
        <PermissionInputSummary input={request.input} />
      </WidgetBody>
      <WidgetActions>
        <button class="widget-btn accept" onClick={handleAllowOnce}>Allow</button>
        <button class={`widget-btn accept ${styles.allowAlways}`} onClick={handleAllowSession}>Always</button>
        <button class="widget-btn deny" onClick={handleDeny}>Deny</button>
      </WidgetActions>
    </div>
  )
}
