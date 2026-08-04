import type { JSX, RefObject } from 'preact'
import { useRef, useEffect } from 'preact/hooks'
import { tabPromptReady, tabPromptVisible, mcpLoading } from '../../signals/connection'
import { activeTabId, tabs } from '../../signals/tabs'

interface PromptTextareaProps {
  textareaRef?: RefObject<HTMLTextAreaElement>
  onInput: (value: string) => void
  onKeyDown: (e: KeyboardEvent) => void
  onPaste: (e: ClipboardEvent) => void
}

function getPlaceholder(disabled: boolean, mcp: boolean, connStatus: string, promptReady: boolean): string {
  if (mcp) return 'Loading MCP tools...'
  if (connStatus === 'disconnected' || connStatus === 'error') return 'Disconnected — waiting for bridge...'
  if (connStatus === 'connecting') return 'Connecting to bridge...'
  if (!promptReady) return 'Claude is working...'
  return 'Type your message... (Enter to send, Shift+Enter for new line)'
}

export function PromptTextarea({ textareaRef: externalRef, onInput, onKeyDown, onPaste }: PromptTextareaProps): JSX.Element {
  const internalRef = useRef<HTMLTextAreaElement>(null)
  const ref = externalRef ?? internalRef

  const mcp = mcpLoading.value
  const tabId = activeTabId.value
  const isPromptReady = tabId ? (tabPromptReady.value.get(tabId) ?? false) : false
  const tab = tabs.value.find(t => t.id === tabId)
  const connStatus = tab?.status ?? 'disconnected'
  const isDisconnected = connStatus === 'disconnected' || connStatus === 'error' || connStatus === 'connecting'
  // Only disable on disconnect — when busy, user can still type for queue
  const disabled = isDisconnected
  const isBusy = !isDisconnected && (!isPromptReady || mcp)

  const placeholder = isBusy
    ? 'Type to queue message... (will send when ready)'
    : getPlaceholder(disabled, mcp, connStatus, isPromptReady)

  function handleInput(): void {
    const el = ref.current
    if (!el) return
    // rAF batches the read+write so we don't thrash layout. Cap the height
    // at 50% of the viewport (down from the old hard-coded 240px) — pasting
    // a whole file should grow the textarea until half the screen is full
    // rather than getting stuck at ~8 lines with a tiny scrollbar. 180px
    // floor keeps the collapsed/empty state from snapping when the user
    // deletes content.
    requestAnimationFrame(() => {
      const max = Math.max(180, Math.floor(window.innerHeight * 0.5))
      el.style.height = 'auto'
      el.style.height = Math.min(el.scrollHeight, max) + 'px'
    })
    onInput(el.value)
  }

  return (
    <textarea
      ref={ref}
      id="prompt-input"
      placeholder={placeholder}
      rows={1}
      disabled={disabled}
      onInput={handleInput}
      onKeyDown={onKeyDown}
      onPaste={onPaste}
    />
  )
}
