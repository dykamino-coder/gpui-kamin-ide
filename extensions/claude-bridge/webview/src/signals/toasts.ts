import { signal } from '@preact/signals'

export interface Toast {
  id: string
  type: 'success' | 'error' | 'info'
  title: string
  message?: string
  duration?: number // ms, 0 = sticky
}

export const toasts = signal<Toast[]>([])

let nextId = 1

// KaminIDE integration: notifications go to the IDE's SHARED notification UI
// (vscode.window.show*Message via the `show-notification` bridge channel), NOT a
// Bridge-owned toast overlay. `showToast` forwards there when the bridge is
// present; the local `toasts` signal + ToastContainer stay only as the standalone
// dashboard fallback (unused in KaminIDE — ToastContainer is not mounted there).
export function showToast(opts: Omit<Toast, 'id'>): string {
  const bridge = (window as { electronBridge?: { showNotification?: (severity: string, message: string) => void } }).electronBridge
  if (bridge?.showNotification) {
    const severity = opts.type === 'error' ? 'error' : 'info'
    bridge.showNotification(severity, opts.message ? `${opts.title}: ${opts.message}` : opts.title)
    return ''
  }
  const id = `toast-${nextId++}`
  toasts.value = [...toasts.value, { ...opts, id }]
  const duration = opts.duration ?? 5000
  if (duration > 0) {
    setTimeout(() => dismissToast(id), duration)
  }
  return id
}

export function dismissToast(id: string): void {
  toasts.value = toasts.value.filter(t => t.id !== id)
}
