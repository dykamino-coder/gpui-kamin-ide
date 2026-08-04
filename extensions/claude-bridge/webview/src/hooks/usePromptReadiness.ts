import { useCallback, useRef } from 'preact/hooks'
import { tabPromptReady, lastSendAt } from '../signals/connection'

const ANSI_RE = /\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07/g

export interface PromptReadinessHandlers {
  detectPromptInOutput: (tabId: string, data: string) => void
  cancelDebounce: () => void
  markSent: (tabId: string) => void
}

export function usePromptReadiness(): PromptReadinessHandlers {
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const cancelDebounce = useCallback(() => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current)
      debounceTimerRef.current = null
    }
  }, [])

  const markSent = useCallback((tabId: string) => {
    const next = new Map(lastSendAt.value)
    next.set(tabId, Date.now())
    lastSendAt.value = next
    cancelDebounce()
  }, [cancelDebounce])

  const detectPromptInOutput = useCallback((tabId: string, data: string) => {
    const stripped = data.replace(ANSI_RE, '')

    if (stripped.includes('\u276F')) {
      // Ignore ❯ in the PTY echo within 2s of last send (per-tab)
      const tabLastSend = lastSendAt.value.get(tabId) ?? 0
      if (Date.now() - tabLastSend < 2000) return

      cancelDebounce()
      debounceTimerRef.current = setTimeout(() => {
        const next = new Map(tabPromptReady.value)
        next.set(tabId, true)
        tabPromptReady.value = next
      }, 500)
    }
  }, [cancelDebounce])

  return { detectPromptInOutput, cancelDebounce, markSent }
}
