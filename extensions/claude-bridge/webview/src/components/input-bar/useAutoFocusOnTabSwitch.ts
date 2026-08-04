// Auto-focus the prompt textarea once per tab as soon as it's ready.
// Extracted from InputBar.tsx (Sprint 5 / Stage E1).
//
// Triggers on tab creation (new tab + ready) and tab activation (switch +
// ready). Tab remembers its auto-focused state — if user clicks xterm we
// don't steal focus back on subsequent re-renders. State for the tab
// (promptReady, mcpLoading, etc.) may not have propagated yet when the
// switch event fires, so we retry across [0, 80, 200, 500] ms and bail on
// first non-disabled focusable textarea.

import { useEffect, useRef } from 'preact/hooks'
import { activeTabId } from '../../signals/tabs'

export function useAutoFocusOnTabSwitch(
  tabId: string | undefined,
  textareaRef: { current: HTMLTextAreaElement | null },
): void {
  const lastAutoFocusedTabRef = useRef<string | null>(null)

  useEffect(() => {
    if (!tabId) return
    if (lastAutoFocusedTabRef.current === tabId) return
    const targetTab = tabId
    const delays = [0, 80, 200, 500]
    const timers: ReturnType<typeof setTimeout>[] = []
    for (const d of delays) {
      timers.push(setTimeout(() => {
        if (activeTabId.value !== targetTab) return
        if (lastAutoFocusedTabRef.current === targetTab) return
        const el = textareaRef.current
        if (!el || el.disabled) return
        lastAutoFocusedTabRef.current = targetTab
        el.focus()
      }, d))
    }
    return () => { for (const t of timers) clearTimeout(t) }
  }, [tabId])
}
