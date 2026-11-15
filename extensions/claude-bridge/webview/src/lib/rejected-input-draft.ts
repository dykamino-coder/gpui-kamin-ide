// Native transport refusals belong to the originating tab, even if its shared
// composer has already switched to another session. Nothing here sends input.
import { storage } from './storage'
import { inputDraftPort } from './input-draft-port'

const restorers = new Map<string, (text: string) => void>()
export const appendDraftText = (draft: string, text: string): string => draft ? `${draft}\n${text}` : text

export function bindRejectedInputDraft(tabId: string, restore: (text: string) => void): () => void {
  restorers.set(tabId, restore)
  return () => { if (restorers.get(tabId) === restore) restorers.delete(tabId) }
}

export function restoreRejectedInputDraft(tabId: string, text: string): void {
  // The browser host restores its shared draft once, before fan-out to pages.
  if (!text || inputDraftPort()) return
  const restore = restorers.get(tabId)
  if (restore) { restore(text); return }
  const key = `input-draft:${tabId}`
  storage.setItem(key, appendDraftText(storage.getItem(key) ?? '', text))
}
