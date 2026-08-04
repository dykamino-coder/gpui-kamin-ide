import type { JSX } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import { useSignal } from '@preact/signals'
import { ConfirmModal } from './ConfirmModal'

// Owns the imperative confirm-modal state and exposes `window.__showConfirmModal`
// for call sites across the app. Extracted from App.tsx so each decomposed root
// (sidebar / chat iframe) can mount it independently.
export function ConfirmModalHost(): JSX.Element {
  const confirmModal = useSignal<{
    isOpen: boolean
    title: string
    bodyHtml: string
    confirmLabel: string
    isDanger: boolean
    onConfirm: () => void
  }>({ isOpen: false, title: '', bodyHtml: '', confirmLabel: 'Delete', isDanger: true, onConfirm: () => {} })

  const showConfirmRef = useRef<(opts: {
    title: string; bodyHtml: string; confirmLabel?: string; isDanger?: boolean
  }) => Promise<boolean>>(() => Promise.resolve(false))

  useEffect(() => {
    showConfirmRef.current = (opts) =>
      new Promise<boolean>((resolve) => {
        confirmModal.value = {
          isOpen: true,
          title: opts.title,
          bodyHtml: opts.bodyHtml,
          confirmLabel: opts.confirmLabel ?? 'Delete',
          isDanger: opts.isDanger ?? true,
          onConfirm: () => {
            confirmModal.value = { ...confirmModal.value, isOpen: false }
            resolve(true)
          },
        }
      })
    ;(window as any).__showConfirmModal = showConfirmRef.current
  }, [])

  return (
    <ConfirmModal
      isOpen={confirmModal.value.isOpen}
      title={confirmModal.value.title}
      bodyHtml={confirmModal.value.bodyHtml}
      confirmLabel={confirmModal.value.confirmLabel}
      isDanger={confirmModal.value.isDanger}
      onConfirm={confirmModal.value.onConfirm}
      onCancel={() => { confirmModal.value = { ...confirmModal.value, isOpen: false } }}
    />
  )
}
