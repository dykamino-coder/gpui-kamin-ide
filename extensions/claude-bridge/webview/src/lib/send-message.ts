// The ONE path a message takes from any caller to the CLI. Extracted from
// `useTabSend` so the composer and the command API (`lib/command-api.ts`) cannot
// drift: a scripted send must exercise exactly what a click does, or testing it
// proves nothing about the button.
import type { ElectronBridge } from '../../shared/types'
import { tabPromptReady, lastSendAt } from '../signals/connection'
import { tabActivity } from '../signals/ui'
import { enqueueLocal } from '../signals/queue'

/** Ship `message` to `tabId`'s PTY.
 *
 *  The server owns Ctrl+U → paste → Enter as one serialized transaction. The
 *  webview sends one semantic frame so maintenance cannot occupy the prompt
 *  between separate clear and submit frames. `promptReady` still flips
 *  optimistically so Send becomes Stop without waiting for the server. */
export function sendMessageToTab(bridge: ElectronBridge, tabId: string, message: string): void {
  // Mirror it into the visual queue when the CLI is mid-turn. The message goes
  // to the CLI either way — the CLI queues it itself — so this only decides
  // whether the user SEES it pending. Lives here rather than in the composer so
  // a scripted send queues exactly like a click; while it sat in `InputBar` a
  // command-driven send silently skipped the queue.
  const busy = (tabActivity.value.get(tabId)?.isWorking ?? false) || !(tabPromptReady.value.get(tabId) ?? false)
  if (busy) enqueueLocal(tabId, message)

  const nextSend = new Map(lastSendAt.value)
  nextSend.set(tabId, Date.now())
  lastSendAt.value = nextSend
  const next = new Map(tabPromptReady.value)
  next.set(tabId, false)
  tabPromptReady.value = next

  bridge.submitText(tabId, message)
}
