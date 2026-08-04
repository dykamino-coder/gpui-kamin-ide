// Global ESC → cancel CLI turn + restore queued messages back to the
// textarea.
//
// Reality check: CLI's `handleCancel` (useCancelRequest.ts) runs on BOTH
// Escape and Ctrl+C through keybindings. It cancels active turn (priority
// 1) and otherwise pops the queue back into the CLI input (priority 2).
// PromptInput.tsx ALSO has its own `key.escape` handler that calls
// `popAllCommandsFromQueue` if any queued command is editable, regardless
// of active turn. So:
//   - Esc with active task: popAllCommandsFromQueue (PromptInput) fires →
//     CLI input rehydrated. Plus handleCancel cancels turn.
//   - Esc on idle: popQueue. CLI input rehydrated.
//   - Ctrl+C with active task: handleCancel priority 1 → cancel only,
//     CLI input pristine.
//   - Ctrl+C on idle (e.g. user double-presses, or CLI just finalized):
//     handleCancel priority 2 → popQueue. CLI input rehydrated.
//
// Bridge wants the local queue to live ONLY in our textarea. So:
//   1. Block Esc at xterm level (useTerminal.ts) — never reach CLI as \x1b.
//   2. Send \x03 (Ctrl+C) to the PTY — cancels turn / triggers handleCancel.
//   3. Send \x15 (Ctrl+U) right after — Ink TextInput scrubs the current
//      line. Idempotent: if pristine, no-op; if popQueue rehydrated, clears.
//   4. Restore queued text into Bridge textarea + clearLocalQueue.

import { useEffect } from 'preact/hooks'
import type { ElectronBridge } from '../../shared/types'
import { activeTabId } from '../signals/tabs'
import { tabPromptReady } from '../signals/connection'
import { localQueue, clearLocalQueue } from '../signals/queue'
import { abortStreamingForTab } from '../signals/jsonl'
import { showToast } from '../signals/toasts'

export function useGlobalEscapeCancel(bridge: ElectronBridge): void {
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent): void {
      if (e.key !== 'Escape') return
      const tabId = activeTabId.value
      if (!tabId) return
      const active = document.activeElement as HTMLElement | null
      const tag = active?.tagName
      const isPromptInput = active?.id === 'prompt-input'
      if (!isPromptInput && (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT')) return

      const queued = localQueue.value.get(tabId) ?? []
      const isBusy = !(tabPromptReady.value.get(tabId) ?? true)
      const shouldCancel = isBusy || queued.length > 0
      if (!shouldCancel) return

      e.preventDefault()
      // Freeze the chat's streamed answer, then interrupt for real — aborts the
      // in-flight upstream response server-side so it stops NOW (SIGINT alone let
      // the already-requested SSE keep flushing). See stop() in useTabSend.
      abortStreamingForTab(tabId)
      bridge.interruptSession(tabId) // server aborts upstream + SIGINTs the CLI (no extra \x03 — would double the signal)
      // Scrub CLI input line a tick later — covers the popQueue rehydrate
      // path (handleCancel priority 2 / Esc-driven popAllCommandsFromQueue
      // when the runner already finalized). Ctrl+U is Ink TextInput's
      // standard "clear-line" binding and is a safe no-op on empty input.
      setTimeout(() => bridge.sendInput(tabId, '\x15'), 30)

      if (queued.length === 0) return

      const textarea = document.getElementById('prompt-input') as HTMLTextAreaElement | null
      if (textarea) {
        const current = textarea.value
        const queuedText = queued.map(q => q.content).join('\n\n')
        textarea.value = current.trim()
          ? `${queuedText}\n\n${current}`
          : queuedText
        textarea.dispatchEvent(new Event('input', { bubbles: true }))
        textarea.focus()
        const end = textarea.value.length
        try { textarea.setSelectionRange(end, end) } catch { /* unsupported */ }
      }
      clearLocalQueue(tabId)

      showToast({
        type: 'info',
        title: 'Queue restored',
        message: `${queued.length} message${queued.length === 1 ? '' : 's'} moved back to input`,
        duration: 2500,
      })
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [])
}
