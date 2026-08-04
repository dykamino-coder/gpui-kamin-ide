import type { JSX } from 'preact'
import { useRef, useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import { useSlashAutocomplete } from '../../hooks/useSlashAutocomplete'
import { useInputHistory } from '../../hooks/useInputHistory'
import { useClipboardPaste } from '../../hooks/useClipboardPaste'
import { useVoiceInput } from '../../hooks/useVoiceInput'
import { mcpLoading, tabPromptReady } from '../../signals/connection'
import { tabWorking } from '../../signals/ui'
import { activeTabId, tabs } from '../../signals/tabs'
import { sessionBindingPending } from '../../signals/session-binding'
import { replayProgressByTab } from '../../signals/replay-progress'
import { activeLocalQueue } from '../../signals/queue'
import { AttachmentsArea } from './AttachmentsArea'
import { PromptTextarea } from './PromptTextarea'
import { SendButton } from './SendButton'
import type { SendMode } from './SendButton'
import { AttachButton } from './AttachButton'
import { VoiceButton } from './VoiceButton'
import { InputControls } from './InputControls'
import { SlashAutocomplete } from './SlashAutocomplete'
import { ScrollDownPill } from './ScrollDownPill'
import { ActiveFileStatusBar } from './ActiveFileStatusBar'
import { useTabSend } from './useTabSend'
import { useAutoFocusOnTabSwitch } from './useAutoFocusOnTabSwitch'
import { useAppendToInputBridge } from './useAppendToInputBridge'
import { useInputDraft } from './useInputDraft'

export function InputBar(): JSX.Element {
  const bridge = useBridge()
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const [hasText, setHasText] = useState(false)

  const [slashState, slashHandlers] = useSlashAutocomplete()
  const history = useInputHistory()
  const { onPaste } = useClipboardPaste()
  const send = useTabSend(bridge)

  const [voiceState, voiceHandlers] = useVoiceInput((text) => {
    const el = textareaRef.current
    if (!el) return
    el.value = el.value + text
    el.focus()
    setHasText(true)
  })

  // Show voice button when Whisper URL is configured or mic bridge connected
  useEffect(() => {
    bridge.getConfig().then((cfg: any) => {
      if (cfg.whisperUrl) voiceHandlers.setVisible(true)
    }).catch(() => {})
    // micConnected-проверка Chrome-PWA моста вырезана (аудит #70 A2).
  }, [])

  const mcp = mcpLoading.value
  const tabId = activeTabId.value
  // Server's promptReady flag is authoritative — the xterm AND-gate experiment
  // produced false negatives (Stop button stuck on idle sessions). Kept the
  // signal for potential future use, but don't block Send on it.
  const isPromptReady = tabId ? (tabPromptReady.value.get(tabId) ?? false) : false
  const tab = tabs.value.find(t => t.id === tabId)
  const connStatus = tab?.status ?? 'disconnected'
  const isDisconnected = connStatus === 'disconnected' || connStatus === 'error' || connStatus === 'connecting'
  // CLI sometimes flips promptReady back to true between turns while the
  // activity indicator (Seasoning/Thinking) is still spinning. Treat an
  // active activity spinner as "busy" too — otherwise the send-arrow returns
  // and the user has no way to interrupt a long think.
  const isActivityWorking = tabId ? (tabWorking.value.get(tabId) ?? false) : false
  const isBusy = !isDisconnected && (!isPromptReady || mcp || isActivityWorking)
  // The chat still shows the PREVIOUS session while a big one replays, but
  // `activeTabId` already points at the new one. Sending in that window delivers
  // the message to a session the user cannot see — so hold until the view and
  // the target agree.
  const isSwitching = sessionBindingPending.value
  // A session whose historical transcript is still downloading is not ready for
  // input — the chat paints its first entries (clearing `isSwitching`) well
  // before replay completes, so without this the box went live mid-load, before
  // the compact strip and console had caught up. `replayProgress` is non-null
  // ONLY during that download and self-clears at replay-complete, so this can
  // never wedge the box on (a small/empty session sends no progress → enabled).
  const isLoading = tabId ? replayProgressByTab.value.get(tabId) != null : false
  const isNotReady = isSwitching || isLoading
  const qCount = activeLocalQueue.value.length

  const { clearDraft } = useInputDraft({ tabId: tabId ?? undefined, textareaRef, setHasText })
  useAppendToInputBridge(textareaRef, setHasText)
  useAutoFocusOnTabSwitch(tabId ?? undefined, textareaRef)

  const sendMode: SendMode = (isBusy && !hasText) ? 'stop' : 'send'

  function clearAndReset(): void {
    send.clearInput(textareaRef.current)
    clearDraft()  // sent text must not resurface as a draft on the next mount
    slashHandlers.hide()
    setHasText(false)
  }

  function handleButtonClick(): void {
    if (!tabId) return
    // Stop must work even while the transcript is still downloading — a resumed
    // session can be mid-generation during that 15-20s window, which is exactly
    // when interrupting matters most. Only SEND is gated below.
    if (sendMode === 'stop') { send.stop(tabId); return }

    const message = send.buildMessage(textareaRef.current)
    if (!message) return
    // Never send into the gap between "session selected" and "session shown", or
    // while its transcript is still downloading.
    if (isNotReady) return

    history.push(tabId, message)
    history.reset(tabId)
    // The visual queue is handled inside the shared send path (see
    // `lib/send-message.ts`), so a scripted send queues identically.
    send.sendToTerminal(tabId, message)
    clearAndReset()
  }

  function handleKeyDown(e: KeyboardEvent): void {
    if (slashState.visible) {
      const handled = slashHandlers.onKeyDown(e)
      if (handled) {
        e.preventDefault()
        // Stop Esc from reaching the document-level handler in App.tsx —
        // inside a visible autocomplete Esc means "dismiss", not "cancel
        // CLI task". Without stopPropagation, both fire and we'd interrupt
        // the running agent whenever the user cancels completion.
        e.stopPropagation()
        if (e.key === 'Tab' || e.key === 'Enter') {
          const cmd = slashHandlers.applySelected()
          if (cmd && textareaRef.current) {
            textareaRef.current.value = cmd
            textareaRef.current.focus()
            slashHandlers.hide()
            setHasText(true)
          }
        }
        return
      }
    }
    // ArrowUp/Down — input history navigation (seed from JSONL on first use)
    if ((e.key === 'ArrowUp' || e.key === 'ArrowDown') && tabId) {
      const el = textareaRef.current
      if (!el) return
      history.seedFromEntries(tabId, (window as any).__jsonlEntries?.[tabId] || [])
      const val = el.value
      const pos = el.selectionStart ?? 0
      const atFirstLine = val.slice(0, pos).indexOf('\n') === -1
      const atLastLine = val.slice(pos).indexOf('\n') === -1
      if (e.key === 'ArrowUp' && (val === '' || atFirstLine)) {
        const prev = history.navigate(tabId, 'up', val)
        if (prev !== null) {
          e.preventDefault()
          el.value = prev
          setHasText(!!prev)
          // Native 'input' event so PromptTextarea's handleInput runs its
          // auto-grow rAF — direct `.value =` assignment doesn't fire it,
          // so without this the textarea stayed at its single-line height
          // when navigating to a long historical prompt.
          el.dispatchEvent(new Event('input', { bubbles: true }))
          requestAnimationFrame(() => { el.selectionStart = el.selectionEnd = prev.length })
        }
      } else if (e.key === 'ArrowDown' && (val === '' || atLastLine)) {
        const next = history.navigate(tabId, 'down', val)
        if (next !== null) {
          e.preventDefault()
          el.value = next
          setHasText(!!next)
          el.dispatchEvent(new Event('input', { bubbles: true }))
          requestAnimationFrame(() => { el.selectionStart = el.selectionEnd = next.length })
        }
      }
      return
    }
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleButtonClick()
    }
  }

  function handleInput(value: string): void {
    slashHandlers.onInput(value)
    setHasText(!!value.trim())
  }

  function handleSlashSelect(cmd: string): void {
    if (!textareaRef.current) return
    textareaRef.current.value = cmd
    textareaRef.current.focus()
    slashHandlers.hide()
    setHasText(true)
  }

  return (
    <div class={`input-bar${(isDisconnected || isNotReady) && sendMode !== 'stop' ? ' disabled' : ''}`} id="input-bar" title={isNotReady && sendMode !== 'stop' ? 'Loading the session…' : undefined} style={{ position: 'relative' }}>
      <ScrollDownPill />
      <ActiveFileStatusBar />
      <div class="input-wrapper">
        <AttachmentsArea />
        <div class="input-textarea-row">
          <AttachButton />
          <VoiceButton voiceState={voiceState} onToggle={voiceHandlers.toggle} />
          <PromptTextarea
            textareaRef={textareaRef}
            onInput={handleInput}
            onKeyDown={handleKeyDown}
            onPaste={onPaste}
          />
          <SendButton mode={sendMode} queueCount={qCount} onClick={handleButtonClick} />
        </div>
        <InputControls />
      </div>
      <SlashAutocomplete
        query={slashState.query}
        matches={slashState.matches}
        selectedIndex={slashState.selectedIndex}
        visible={slashState.visible}
        onSelect={handleSlashSelect}
        onHover={slashHandlers.selectIndex}
      />
    </div>
  )
}
