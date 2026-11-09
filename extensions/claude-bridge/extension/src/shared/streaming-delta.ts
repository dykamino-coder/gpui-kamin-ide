/** Shared by the authoritative client cache and its disposable UI projection. */
export interface StreamingTextDelta {
  msgId: string
  blockIdx: number
  appendText: string
  kind: 'text' | 'thinking'
}

export function appendStreamingText(entry: any, delta: StreamingTextDelta): boolean {
  if (!entry || entry.__streaming === undefined || entry.message?.id !== delta.msgId ||
      !Number.isSafeInteger(delta.blockIdx) || delta.blockIdx < 0 || typeof delta.appendText !== 'string') return false
  const block = entry.message?.content?.[delta.blockIdx]
  if (delta.kind === 'text' && block?.type === 'text') block.text = (block.text ?? '') + delta.appendText
  else if (delta.kind === 'thinking' && block?.type === 'thinking') block.thinking = (block.thinking ?? '') + delta.appendText
  else return false
  return true
}
