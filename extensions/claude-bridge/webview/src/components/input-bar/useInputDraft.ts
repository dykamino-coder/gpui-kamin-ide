// Per-session input draft persistence. The composer textarea is a single shared
// uncontrolled element, so without this switching sessions bleeds one tab's typed
// text into another and any unsent text is lost on reload/reconnect. Here each
// session's unsent input is persisted (via the webview `storage`, which is backed
// by the VS Code webview state API and survives an iframe reload — the reconnect
// path in bridge-shim hard-reloads the iframe) and restored on tab switch / mount.
//
// - On tabId change: flush the OUTGOING tab's draft (effect cleanup) and load the
//   INCOMING tab's draft into the textarea.
// - On input: debounce-save so text survives a hard reload mid-typing.
// - On send: clearDraft() cancels the pending save and removes the stored draft.
//
// The load runs in useLayoutEffect (not useEffect): the textarea is uncontrolled,
// so its only writer is this hook. A post-paint useEffect would let the OUTGOING
// session's draft stay painted for a frame after the message list already swapped
// to the incoming session (both react to activeTabId, but the list re-renders in
// the same commit while a passive effect lands a frame later) — visible as the
// old text "lingering". A layout effect swaps the value before paint, in lockstep
// with the rest of the switch.
//
// Attached files are part of the draft too (owner, 2026-10-13): `pendingAttachments`
// holds the ACTIVE session's attachments; on switch the outgoing session's list is
// stored under its id and the incoming one's is restored, so a file attached in one
// chat no longer travels to another.

import { useLayoutEffect, useRef, useCallback } from 'preact/hooks'
import { storage } from '../../lib/storage'
import { pendingAttachments, type FileAttachment } from '../../signals/ui'

const DRAFT_PREFIX = 'input-draft:'
const ATTACH_PREFIX = 'input-attachments:'
const SAVE_DEBOUNCE_MS = 400
const key = (tabId: string): string => DRAFT_PREFIX + tabId
const attachKey = (tabId: string): string => ATTACH_PREFIX + tabId

export function loadAttachments(tabId: string): FileAttachment[] {
  try {
    const raw = storage.getItem(attachKey(tabId))
    const list: unknown = raw ? JSON.parse(raw) : []
    return Array.isArray(list) ? (list as FileAttachment[]) : []
  } catch {
    return []
  }
}

export function saveAttachments(tabId: string, list: FileAttachment[]): void {
  try {
    if (list.length) storage.setItem(attachKey(tabId), JSON.stringify(list))
    else storage.removeItem(attachKey(tabId))
  } catch {
    // Storage full (large pasted images): the in-memory list still works for
    // this session; only reload persistence is lost.
  }
}

interface Args {
  tabId: string | undefined
  textareaRef: { current: HTMLTextAreaElement | null }
  setHasText: (v: boolean) => void
}

export function useInputDraft({ tabId, textareaRef, setHasText }: Args): { clearDraft: () => void } {
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  useLayoutEffect(() => {
    if (!tabId) return
    const el = textareaRef.current

    // Load this session's saved draft (empty clears the shared textarea).
    if (el) {
      const draft = storage.getItem(key(tabId)) ?? ''
      el.value = draft
      setHasText(!!draft.trim())
      // Fire a native 'input' so PromptTextarea's auto-grow rAF sizes the box to
      // the restored draft (direct `.value =` doesn't trigger it).
      if (draft) requestAnimationFrame(() => el.dispatchEvent(new Event('input', { bubbles: true })))
    }
    pendingAttachments.value = loadAttachments(tabId)
    // Persist every change of this session's attachments (add/remove/send).
    // `subscribe` fires once immediately with the value just loaded — skip it.
    let primed = false
    const unsubscribe = pendingAttachments.subscribe(list => {
      if (primed) saveAttachments(tabId, list)
      primed = true
    })

    const flush = (): void => {
      const v = textareaRef.current?.value ?? ''
      if (v.trim()) storage.setItem(key(tabId), v)
      else storage.removeItem(key(tabId))
    }
    const onInput = (): void => {
      if (saveTimer.current) clearTimeout(saveTimer.current)
      saveTimer.current = setTimeout(flush, SAVE_DEBOUNCE_MS)
    }
    el?.addEventListener('input', onInput)

    return () => {
      unsubscribe()
      saveAttachments(tabId, pendingAttachments.value)
      el?.removeEventListener('input', onInput)
      if (saveTimer.current) { clearTimeout(saveTimer.current); saveTimer.current = null }
      flush() // persist the outgoing tab's draft synchronously on switch/unmount
    }
  }, [tabId])

  const clearDraft = useCallback((): void => {
    if (saveTimer.current) { clearTimeout(saveTimer.current); saveTimer.current = null }
    if (tabId) {
      storage.removeItem(key(tabId))
      storage.removeItem(attachKey(tabId))
    }
  }, [tabId])

  return { clearDraft }
}
