// Send/queue/stop logic extracted from InputBar.tsx (Sprint 5 / Stage E1).
// Owns the bridge dance for shipping a textarea's contents to the active
// PTY tab: Ctrl+U scrub → bracketed paste with a 50ms gap, optimistic
// promptReady flip so the Send button transitions to Stop without waiting
// for the server, and pending-Enter timer cleanup so a fast double-send
// doesn't smear two messages together.

import type { ElectronBridge } from '../../../shared/types'
import { pendingAttachments as attachSignal } from '../../signals/ui'
import { activeSelection, attachActiveFile } from '../../signals/file-viewer'
import { abortStreamingForTab } from '../../signals/jsonl'
import { sendMessageToTab } from '../../lib/send-message'

export interface TabSendHandle {
  buildMessage: (textarea: HTMLTextAreaElement | null) => string | null
  sendToTerminal: (tabId: string, message: string) => void
  stop: (tabId: string) => void
  clearInput: (textarea: HTMLTextAreaElement | null) => void
}

export function useTabSend(bridge: ElectronBridge): TabSendHandle {
  function buildMessage(el: HTMLTextAreaElement | null): string | null {
    if (!el) return null
    const text = el.value
    const attachments = attachSignal.value
    // When the auto-attach switch is on, append a single fenced JSON
    // block describing the editor context. The semantics of the
    // `editor-context` tag — what each field means and how the agent
    // should treat it — are documented once in the per-session
    // CLAUDE.md (see BRIDGE_DEFAULT_CLAUDE_MD), so we don't need a
    // per-message header anymore.
    let fileContext = ''
    if (attachActiveFile.value) {
      const sel = activeSelection.value
      if (sel) {
        const payload = {
          openedFile: sel.path,
          selectedRange:
            sel.startLine === sel.endLine
              ? { line: sel.startLine }
              : { startLine: sel.startLine, endLine: sel.endLine },
          textInSelectedRange: sel.text || null,
        }
        fileContext = '\n\n```json editor-context\n' + JSON.stringify(payload, null, 2) + '\n```'
      }
    }
    const trimmed = text.trim()
    if (!trimmed && !fileContext && attachments.length === 0) return null
    const imagePaths = attachments.length > 0 ? '\n' + attachments.map(a => a.path).join('\n') : ''
    if (!trimmed && fileContext) return fileContext.trimStart() + imagePaths
    return text + fileContext + imagePaths
  }

  function clearInput(el: HTMLTextAreaElement | null): void {
    if (el) {
      el.value = ''
      el.style.height = 'auto'
    }
    attachSignal.value = []
  }

  function sendToTerminal(tabId: string, message: string): void {
    sendMessageToTab(bridge, tabId, message)
  }

  function stop(tabId: string): void {
    // Freeze the chat's streamed answer at the current text FIRST (gates future
    // stream frames locally).
    abortStreamingForTab(tabId)
    // Then interrupt for real: the server aborts the in-flight upstream Anthropic
    // response (so the stream stops NOW instead of flushing to completion) AND
    // SIGINTs the CLI to end the turn. Do NOT also send \x03 here — that would
    // deliver SIGINT twice, and a second cancel signal can pop the input queue.
    bridge.interruptSession(tabId)
  }

  return { buildMessage, sendToTerminal, stop, clearInput }
}
