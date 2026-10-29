import type { JSX } from 'preact'
import type { JsonlEntryData } from '../../types/jsonl'
import { submissions, dismissUncertain } from '../../signals/submissions'
import { activeTabId } from '../../signals/tabs'
import { useBridge } from '../../hooks/useBridge'

export function SubmissionBadge({ entry }: { entry: JsonlEntryData }): JSX.Element | null {
  const bridge = useBridge()
  const tabId = activeTabId.value
  const item = tabId ? submissions.value.get(tabId)?.find(i => i.id === entry.submissionId || i.uuid === entry.uuid) : undefined
  if (!item || !tabId) return null
  const read = item.state === 'read'
  const uncertain = item.state === 'uncertain'
  const label = read ? 'Read' : uncertain ? 'Delivery unconfirmed' : 'Queued'
  const action = (name: 'cancel' | 'push' | 'interrupt') => bridge.submissionAction(tabId, item.id, name)
  return <span class="submission-status" data-submission-id={item.id} data-state={item.state}
    title={item.warning} style="margin-left:8px;display:inline-flex;gap:8px;align-items:center;font-size:11px;color:var(--text-secondary)">
    <span>{label}</span>
    {item.warning && <span role="status">{item.warning}</span>}
    {uncertain && <button title="Hide the unconfirmed row; this does not cancel CLI delivery" onClick={() => dismissUncertain(tabId, item.id)}>Dismiss</button>}
    {item.cancelable && <button onClick={() => action('cancel')}>Cancel</button>}
    {item.state === 'queued' && <button onClick={() => action('push')}>Send now</button>}
    {(item.state === 'queued' || item.state === 'native') &&
      <button title="Interrupt the current turn and process the queued message" onClick={() => action('interrupt')}>Interrupt and send</button>}
  </span>
}
