import type { JSX } from 'preact'
import { activeLocalQueue, localQueue } from '../../signals/queue'
import { activeTabId } from '../../signals/tabs'
import { tabWorking } from '../../signals/ui'
import { useBridge } from '../../hooks/useBridge'

function hasAttachment(msg: string): boolean {
  return /\.(png|jpg|jpeg|gif|webp|svg|pdf)$/im.test(msg)
}

function truncateMsg(msg: string): string {
  const lines = msg.split('\n').filter(l => !/^\/.+\.(png|jpg|jpeg|gif|webp)$/i.test(l) && !/^[A-Z]:\\.+\.(png|jpg|jpeg|gif|webp)$/i.test(l))
  const text = lines.join('\n').trim() || '(image attachment)'
  return text.length > 200 ? text.slice(0, 200) + '...' : text
}

export function QueueWidget(): JSX.Element | null {
  const bridge = useBridge()
  const queue = activeLocalQueue.value
  const tabId = activeTabId.value
  const working = tabId ? (tabWorking.value.get(tabId) ?? false) : false
  if (queue.length === 0) return null

  /** End the running turn so the CLI reaches its prompt and takes the queue.
   *
   *  Interrupt ONLY — deliberately no re-send. The message is already with the
   *  CLI (the composer sends it; this widget merely mirrors it for display) and
   *  the CLI keeps its pending input across the interrupt: measured, the queued
   *  text lands as a normal user turn right after `[Request interrupted by
   *  user]`. An added re-send therefore delivered it three times over. */
  function pushThrough(): void {
    if (!tabId) return
    bridge.interruptSession(tabId)
  }

  function removeItem(id: string): void {
    const tabId = activeTabId.value
    if (!tabId) return
    const list = localQueue.value.get(tabId)
    if (!list) return
    const next = new Map(localQueue.value)
    const kept = list.filter(i => i.id !== id)
    if (kept.length === 0) next.delete(tabId)
    else next.set(tabId, kept)
    localQueue.value = next
  }

  return (
    <div class="widget-card">
      <div class="widget-header">
        <span class="widget-icon"><i class="fas fa-layer-group" /></span>
        <span class="widget-title">Message Queue</span>
        <span class="widget-type-badge">{queue.length} pending</span>
        {working && (
          <button
            style="margin-left:auto;background:none;border:1px solid var(--bg-overlay);color:var(--text-secondary);cursor:pointer;font-size:11px;padding:2px 8px;border-radius:var(--radius-xs);display:inline-flex;align-items:center;gap:5px"
            data-tooltip="Stop the current turn and send the queued messages right away"
            onClick={pushThrough}
          >
            <i class="fas fa-forward" style="font-size:10px" />
            Send now
          </button>
        )}
      </div>
      <div style="padding:8px 14px;display:flex;flex-direction:column;gap:6px">
        {queue.map((item, i) => (
          <div
            key={item.id}
            style="display:flex;align-items:flex-start;gap:8px;padding:6px 10px;background:var(--bg-primary);border:1px solid var(--bg-surface);border-radius:var(--radius-sm)"
          >
            <span style="color:var(--text-disabled);font-size:11px;flex-shrink:0;margin-top:2px">#{i + 1}</span>
            <span style="flex:1;font-size:12px;color:var(--text-primary);white-space:pre-wrap;word-break:break-word;max-height:60px;overflow:hidden">
              {hasAttachment(item.content) && <i class="fas fa-paperclip" style="color:var(--text-disabled);margin-right:4px;font-size:10px" />}
              {truncateMsg(item.content)}
            </span>
            <button
              style="background:none;border:none;color:var(--text-disabled);cursor:pointer;font-size:11px;flex-shrink:0;padding:2px 4px"
              data-tooltip="Remove from queue"
              onClick={() => removeItem(item.id)}
            >
              <i class="fas fa-xmark" />
            </button>
          </div>
        ))}
        <span style="align-self:flex-end;font-size:10px;color:var(--text-disabled)">Cleared once the CLI picks up the message</span>
      </div>
    </div>
  )
}
