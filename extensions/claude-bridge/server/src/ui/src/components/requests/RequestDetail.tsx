import { selectedRequest } from '../../signals/requests'
import { Modal, Badge } from '../shared'
import { ApiFormatBadge } from './ApiFormatBadge'
import styles from './RequestDetail.module.css'

interface RequestDetailProps {
  isOpen: boolean
  onClose: () => void
}

export function RequestDetail({ isOpen, onClose }: RequestDetailProps) {
  const req = selectedRequest.value
  if (!req) return null

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={`Request #${req.id.slice(0, 8)}`} size="lg">
      <div class={styles.grid}>
        <div class={styles.field}>
          <span class={styles.label}>Endpoint</span>
          <span class={styles.value}>{req.method}</span>
        </div>
        <div class={styles.field}>
          <span class={styles.label}>Format</span>
          <ApiFormatBadge endpoint={req.endpoint} size="md" />
        </div>
        <div class={styles.field}>
          <span class={styles.label}>Model</span>
          <span class={styles.value}>{req.model}</span>
        </div>
        <div class={styles.field}>
          <span class={styles.label}>Plugin</span>
          <span class={styles.value}>{req.pluginId}</span>
        </div>
        {req.sessionKey && (
          <div class={styles.field}>
            <span class={styles.label}>Session Key</span>
            <span class={styles.value} title={req.sessionKey}>{req.sessionKey}</span>
          </div>
        )}
        <div class={styles.field}>
          <span class={styles.label}>Duration</span>
          <span class={styles.value}>{req.durationMs}ms</span>
        </div>
        <div class={styles.field}>
          <span class={styles.label}>Status</span>
          <Badge variant={req.status === 'error' ? 'error' : req.status === 'streaming' ? 'info' : 'success'}>
            {req.statusCode} {req.status}
          </Badge>
        </div>
        <div class={styles.field}>
          <span class={styles.label}>Input Tokens</span>
          <span class={styles.value}>{req.inputTokens}</span>
        </div>
        <div class={styles.field}>
          <span class={styles.label}>Output Tokens</span>
          <span class={styles.value}>{req.outputTokens}</span>
        </div>
      </div>

      {req.userMessage && (
        <div class={styles.section}>
          <h4 class={styles.sectionTitle}>User Message</h4>
          <pre class={styles.codeBlock}>{req.userMessage}</pre>
        </div>
      )}

      {req.toolsUsed.length > 0 && (
        <div class={styles.section}>
          <h4 class={styles.sectionTitle}>Tools Used</h4>
          <div class={styles.tools}>
            {req.toolsUsed.map((t: string) => (
              <Badge key={t} variant="default" size="sm">{t}</Badge>
            ))}
          </div>
        </div>
      )}

      {req.error && (
        <div class={styles.section}>
          <h4 class={styles.sectionTitle}>Error</h4>
          <pre class={styles.errorBlock}>{req.error}</pre>
        </div>
      )}

      {req.responseText && (
        <div class={styles.section}>
          <h4 class={styles.sectionTitle}>{req.endpoint === 'pty' ? 'Session Text' : 'Response'}</h4>
          <pre class={styles.codeBlock} style={{ maxHeight: '400px', overflow: 'auto' }}>{req.responseText}</pre>
        </div>
      )}
    </Modal>
  )
}
