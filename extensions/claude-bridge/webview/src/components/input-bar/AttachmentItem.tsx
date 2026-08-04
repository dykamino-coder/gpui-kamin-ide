import type { JSX } from 'preact'
import type { FileAttachment } from '../../signals/ui'
import { AttachmentRemoveButton } from './AttachmentRemoveButton'

const EXT_ICONS: Record<string, string> = {
  pdf: 'fa-file-pdf', doc: 'fa-file-word', docx: 'fa-file-word',
  xls: 'fa-file-excel', xlsx: 'fa-file-excel', csv: 'fa-file-csv',
  txt: 'fa-file-lines', md: 'fa-file-lines', log: 'fa-file-lines',
  json: 'fa-file-code', xml: 'fa-file-code', yaml: 'fa-file-code', yml: 'fa-file-code',
  js: 'fa-file-code', ts: 'fa-file-code', tsx: 'fa-file-code', jsx: 'fa-file-code',
  py: 'fa-file-code', rs: 'fa-file-code', go: 'fa-file-code', java: 'fa-file-code',
  html: 'fa-file-code', css: 'fa-file-code', scss: 'fa-file-code',
  zip: 'fa-file-zipper', tar: 'fa-file-zipper', gz: 'fa-file-zipper',
}

interface AttachmentItemProps {
  attachment: FileAttachment
  onRemove: () => void
}

export function AttachmentItem({ attachment, onRemove }: AttachmentItemProps): JSX.Element {
  const isImage = !!attachment.dataUri

  return (
    <div class="attachment-item" data-tooltip={attachment.path}>
      {isImage ? (
        <img src={attachment.dataUri} alt={attachment.name} />
      ) : (
        <div style="display:flex;align-items:center;gap:6px;padding:4px 8px;background:var(--bg-primary);border-radius:var(--radius-xs);max-width:180px">
          <i class={`fas ${EXT_ICONS[attachment.ext] || 'fa-file'}`} style="color:var(--text-muted);font-size: var(--fs-md);flex-shrink:0" />
          <span style="font-size:11px;color:var(--text-secondary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
            {attachment.name}
          </span>
        </div>
      )}
      <AttachmentRemoveButton onClick={onRemove} />
    </div>
  )
}
