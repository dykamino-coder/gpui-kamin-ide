// Контракт черновика: только текст и ссылки на уже загруженные файлы.
export interface WorkspaceDraft {
  text: string
  attachments: { path: string; name: string; ext: string; dataUri?: string }[]
}
export interface WorkspaceDraftState { revision: number; value: WorkspaceDraft }
export interface WorkspaceDraftReply { accepted: boolean; state: WorkspaceDraftState }
export const emptyDraft = (): WorkspaceDraft => ({ text: '', attachments: [] })
