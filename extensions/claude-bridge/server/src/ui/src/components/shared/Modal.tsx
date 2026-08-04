import { ComponentChildren } from 'preact'
import { useEffect } from 'preact/hooks'
import styles from './Modal.module.css'

interface ModalProps {
  isOpen: boolean
  onClose: () => void
  title?: string
  children: ComponentChildren
  footer?: ComponentChildren
  size?: 'sm' | 'md' | 'lg' | 'xl'
  closeOnOverlay?: boolean
}

export function Modal({
  isOpen,
  onClose,
  title,
  children,
  footer,
  size = 'md',
  closeOnOverlay = true,
}: ModalProps) {
  useEffect(() => {
    if (!isOpen) return
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handleEscape)
    document.body.style.overflow = 'hidden'
    return () => {
      document.removeEventListener('keydown', handleEscape)
      document.body.style.overflow = ''
    }
  }, [isOpen, onClose])

  if (!isOpen) return null

  return (
    <div class={styles.overlay} onClick={closeOnOverlay ? onClose : undefined}>
      <div class={`${styles.modal} ${styles[size]}`} onClick={(e) => e.stopPropagation()}>
        {title && (
          <div class={styles.header}>
            <h3 class={styles.title}>{title}</h3>
            <button class={styles.closeBtn} onClick={onClose}>
              <i class="fa-solid fa-xmark" />
            </button>
          </div>
        )}
        <div class={styles.body}>{children}</div>
        {footer && <div class={styles.footer}>{footer}</div>}
      </div>
    </div>
  )
}
