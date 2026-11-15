// The ONE path a message takes from any caller to the CLI. Extracted from
// `useTabSend` so the composer and the command API (`lib/command-api.ts`) cannot
// drift: a scripted send must exercise exactly what a click does, or testing it
// proves nothing about the button.
import type { KaminBridgeApi } from '../../shared/types'
import { tabPromptReady, lastSendAt } from '../signals/connection'
import { optimisticSubmission } from '../signals/submissions'

/** Ship `message` to `tabId`'s PTY.
 *
 *  The server owns Ctrl+U → paste → Enter as one serialized transaction. The
 *  webview sends one semantic frame so maintenance cannot occupy the prompt
 *  between separate clear and submit frames. `promptReady` still flips
 *  optimistically so Send becomes Stop without waiting for the server. */
export function sendMessageToTab(bridge: KaminBridgeApi, tabId: string, message: string): void {
  // Every prompt needs an identity, including before capabilities arrive: a
  // local refusal must remove exactly its optimistic row. Native CLI delivery
  // already reports receipts/fallback with this same ID.
  const submissionId = optimisticSubmission(tabId, message)

  const nextSend = new Map(lastSendAt.value)
  nextSend.set(tabId, Date.now())
  lastSendAt.value = nextSend
  const next = new Map(tabPromptReady.value)
  next.set(tabId, false)
  tabPromptReady.value = next

  bridge.submitText(tabId, message, submissionId)
}
