// Браузер устанавливает порт хостового хранения до монтирования общего composer.
import type { WorkspaceDraft } from '../../shared/workspace-draft'
export interface InputDraftPort {
  watch(tabId: string, apply: (value: WorkspaceDraft) => void): () => void
  save(tabId: string, value: WorkspaceDraft): void
  ready(tabId: string): boolean
}
let port: InputDraftPort | undefined
export const inputDraftPort = (): InputDraftPort | undefined => port
export function installInputDraftPort(value: InputDraftPort): void { port = value }
