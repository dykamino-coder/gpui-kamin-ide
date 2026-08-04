import type { JSX } from 'preact'

interface AttachmentRemoveButtonProps {
  onClick: () => void
}

export function AttachmentRemoveButton({ onClick }: AttachmentRemoveButtonProps): JSX.Element {
  return (
    <button class="attachment-remove" title="Remove" type="button" onClick={onClick}>
      &times;
    </button>
  )
}
