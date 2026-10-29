import type { JSX } from 'preact'
import type { JsonlEntryData } from '../../types/jsonl'
import { submissions, dismissUncertain } from '../../signals/submissions'
import { activeTabId } from '../../signals/tabs'
import { useBridge } from '../../hooks/useBridge'
import { Button } from '../ui/Button'
import styles from './SubmissionBadge.module.css'

export function SubmissionBadge({ entry }: { entry: JsonlEntryData }): JSX.Element | null {
  const bridge = useBridge()
  const tabId = activeTabId.value
  const item = tabId ? submissions.value.get(tabId)?.find(i => i.id === entry.submissionId || i.uuid === entry.uuid) : undefined
  if (!item || !tabId) return null
  const read = item.state === 'read'
  const uncertain = item.state === 'uncertain'
  const label = read ? 'Read' : uncertain ? 'Unconfirmed' : item.state === 'sending' ? 'Sending' : item.state === 'fallback' ? 'Sent' : 'Queued'
  const explanation = item.warning || (read ? 'CLI has received this message in its conversation.' : item.state === 'native' ? 'Queued in CLI for its next step.' : item.state === 'sending' ? 'Delivering this message to CLI.' : 'Waiting for delivery to CLI.')
  const action = (name: 'cancel' | 'push' | 'interrupt') => bridge.submissionAction(tabId, item.id, name)
  return <span class={`submission-status ${styles.status}`} data-submission-id={item.id} data-state={item.state}>
    <span role="status" title={explanation}>{label}</span>
    {uncertain && <Button class={styles.action} title="Hide the unconfirmed row; this does not cancel CLI delivery" onClick={() => dismissUncertain(tabId, item.id)}>Dismiss</Button>}
    {item.cancelable && <Button class={styles.action} title="Cancel this message before CLI delivery" onClick={() => action('cancel')}>Cancel</Button>}
    {item.state === 'queued' && <Button class={styles.action} title="Deliver this message when the terminal composer is clear" onClick={() => action('push')}>Send now</Button>}
    {(item.state === 'queued' || item.state === 'native') &&
      <Button class={styles.action} title="Interrupt the current turn and process the queued message" onClick={() => action('interrupt')}>Interrupt and send</Button>}
  </span>
}
