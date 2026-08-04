import type { JSX } from 'preact'
import { useEffect, useMemo, useRef } from 'preact/hooks'
import styles from './ConfirmModal.module.css'

/** Strip script/event-handler XSS vectors from HTML while keeping safe markup */
function sanitizeHtml(html: string): string {
  return html
    .replace(/<script[\s\S]*?<\/script>/gi, '')
    .replace(/<script[\s\S]*?>/gi, '')
    .replace(/on\w+\s*=\s*"[^"]*"/gi, '')
    .replace(/on\w+\s*=\s*'[^']*'/gi, '')
    .replace(/on\w+\s*=\s*[^\s>]*/gi, '')
    .replace(/javascript\s*:/gi, '')
}

interface ConfirmModalProps {
  isOpen: boolean
  title: string
  bodyHtml: string
  confirmLabel?: string
  isDanger?: boolean
  onConfirm: () => void
  onCancel: () => void
}

export function ConfirmModal({
  isOpen,
  title,
  bodyHtml,
  confirmLabel = 'Delete',
  isDanger = true,
  onConfirm,
  onCancel,
}: ConfirmModalProps): JSX.Element | null {
  const confirmBtnRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (isOpen && confirmBtnRef.current) {
      confirmBtnRef.current.focus()
    }
  }, [isOpen])

  const safeBodyHtml = useMemo(() => sanitizeHtml(bodyHtml), [bodyHtml])

  if (!isOpen) return null

  function handleOverlayClick(e: MouseEvent): void {
    if (e.target === e.currentTarget) onCancel()
  }

  return (
    <div class={styles.overlay} onClick={handleOverlayClick}>
      <div class={styles.dialog}>
        <h3 class={styles.title}>{title}</h3>
        <p
          class={styles.body}
          dangerouslySetInnerHTML={{ __html: safeBodyHtml }}
        />
        <div class={styles.actions}>
          <button class={styles.cancelBtn} onClick={onCancel}>
            Cancel
          </button>
          <button
            ref={confirmBtnRef}
            class={`${styles.confirmBtn} ${isDanger ? styles.danger : ''}`}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  )
}
