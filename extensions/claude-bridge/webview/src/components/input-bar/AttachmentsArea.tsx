import type { JSX } from 'preact'
import { pendingAttachments } from '../../signals/ui'
import { AttachmentItem } from './AttachmentItem'

export function AttachmentsArea(): JSX.Element {
  const attachments = pendingAttachments.value

  function removeAttachment(index: number): void {
    const next = [...pendingAttachments.value]
    next.splice(index, 1)
    pendingAttachments.value = next
  }

  return (
    <div class={`attachments-area${attachments.length > 0 ? ' has-items' : ''}`} id="attachments-area">
      {attachments.map((a, i) => (
        <AttachmentItem
          key={i}
          attachment={a}
          onRemove={() => removeAttachment(i)}
        />
      ))}
    </div>
  )
}
