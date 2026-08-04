import { capturedLog } from '../../signals/requests'
import { Card } from '../shared'
import { RequestTable } from './RequestTable'
import styles from './RequestCapture.module.css'

export function RequestCapture() {
  const log = capturedLog.value

  return (
    <Card noPadding className={styles.card}>
      {log.length > 0 && (
        <div class={styles.toolbar}>
          <span class={styles.count}>{log.length} requests</span>
        </div>
      )}

      <RequestTable
        entries={log}
        maxHeight={400}
        emptyMessage="No requests yet"
      />
    </Card>
  )
}
