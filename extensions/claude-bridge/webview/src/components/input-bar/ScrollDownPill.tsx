import type { JSX } from 'preact'
import { chatAtBottom, terminalAtBottom, jsonlVisible, viewerVisible } from '../../signals/ui'
import { activeTabId } from '../../signals/tabs'
import { terminalRegistry } from '../../signals/terminal-registry'

export function ScrollDownPill(): JSX.Element | null {
  const chatShown = jsonlVisible.value
  const termShown = viewerVisible.value
  // Re-read signals so Preact tracks them
  const chatAt = chatAtBottom.value
  const termAt = terminalAtBottom.value
  // Active terminal can be ahead of the signal (onScroll not fired yet after
  // tab switch) — peek the current buffer directly as a secondary check.
  let termReallyAt = termAt
  if (termShown) {
    const entry = terminalRegistry.get(activeTabId.value || '')
    if (entry) {
      const buf = entry.terminal.buffer.active
      termReallyAt = buf.viewportY >= buf.baseY
    }
  }
  const chatBehind = chatShown && !chatAt
  const termBehind = termShown && !termReallyAt
  if (!chatBehind && !termBehind) return null

  function scrollAll(): void {
    if (chatBehind) (window as any).__scrollChatToBottom?.()
    if (termBehind) {
      const entry = terminalRegistry.get(activeTabId.value || '')
      entry?.terminal.scrollToBottom()
      terminalAtBottom.value = true
    }
  }

  return (
    <div style="position:absolute;left:0;right:0;top:-36px;display:flex;justify-content:center;pointer-events:none;z-index:10">
      <button
        type="button"
        onClick={scrollAll}
        style="pointer-events:auto;background:var(--tint-surface-medium);backdrop-filter:blur(6px);-webkit-backdrop-filter:blur(6px);border:1px solid var(--tint-overlay-medium);color:var(--text-primary);padding:4px 14px;border-radius:999px;font-size:12px;cursor:pointer;display:inline-flex;align-items:center;gap:6px;box-shadow:var(--shadow-mini);transition:background .15s,border-color .15s,opacity .15s;opacity:0.75"
        onMouseEnter={(e) => { const b = e.currentTarget as HTMLButtonElement; b.style.background = 'var(--tint-overlay-strong)'; b.style.opacity = '1' }}
        onMouseLeave={(e) => { const b = e.currentTarget as HTMLButtonElement; b.style.background = 'var(--tint-surface-medium)'; b.style.opacity = '0.75' }}
      >
        <i class="fas fa-arrow-down" style="font-size:10px" />
        Scroll down
      </button>
    </div>
  )
}
