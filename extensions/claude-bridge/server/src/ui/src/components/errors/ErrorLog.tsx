import type { RequestLogEntry } from '../../types'
import { api } from '../../services/api-client'
import styles from './ErrorLog.module.css'

interface ErrorLogProps {
  errors: RequestLogEntry[]
  onClear: () => void
}

function downloadJson(data: unknown, filename: string) {
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

async function handleDownload(id: string, timestamp: string) {
  try {
    const entry = await api.getRequest(id)
    if (!entry?.requestBody) {
      alert('No request body available for this error')
      return
    }
    const dateStr = new Date(timestamp).toISOString().replace(/[:.]/g, '-').slice(0, 19)
    downloadJson(entry.requestBody, `error_${dateStr}.json`)
  } catch {
    alert('Failed to fetch request details')
  }
}

export function ErrorLog({ errors, onClear }: ErrorLogProps) {
  if (errors.length === 0) {
    return (
      <div class={styles.container}>
        <div class={styles.empty}>No errors recorded</div>
      </div>
    )
  }

  return (
    <div class={styles.container}>
      <div class={styles.header}>
        <span class={styles.headerLeft}>{errors.length} error{errors.length !== 1 ? 's' : ''}</span>
        <button class={styles.clearBtn} onClick={onClear}>
          <i class="fa-solid fa-trash" style={{ marginRight: '4px' }} />
          Clear
        </button>
      </div>
      {errors.map((e) => {
        const time = new Date(e.timestamp).toLocaleString()
        const dur = e.durationMs > 0 ? `${(e.durationMs / 1000).toFixed(1)}s` : ''
        return (
          <div key={e.id} class={styles.row}>
            <div class={styles.rowTop}>
              <span class={styles.time}>{time}</span>
              {e.sessionKey && (
                <span class={styles.session} title={e.sessionKey}>{e.sessionKey.substring(0, 8)}</span>
              )}
              <span class={`${styles.endpoint} ${styles[e.endpoint]}`}>{e.endpoint}</span>
              <span class={styles.model}>{e.model}</span>
              {dur && <span class={styles.duration}>{dur}</span>}
              <span class={styles.status}>{e.statusCode}</span>
              <button
                class={styles.downloadBtn}
                onClick={() => handleDownload(e.id, e.timestamp)}
                title="Download request body"
              >
                <i class="fa-solid fa-download" />
              </button>
            </div>
            {e.error && <div class={styles.errorMsg}>{e.error}</div>}
          </div>
        )
      })}
    </div>
  )
}
