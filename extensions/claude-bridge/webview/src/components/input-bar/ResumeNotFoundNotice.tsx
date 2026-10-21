import type { JSX } from 'preact'
import { WidgetHeader } from '../widgets/WidgetHeader'
import { WidgetBody } from '../widgets/WidgetBody'
import { WidgetActions } from '../widgets/WidgetActions'
import { WidgetButton } from '../widgets/WidgetButton'
import styles from './ResumeNotFoundNotice.module.css'

/** Choice card shown while a restored conversation is missing on the server
 *  (INC-2026-0074). Same widget card as permission prompts, so it reads as part
 *  of the chat rather than a raw browser form. */
export function ResumeNotFoundNotice({ onNew, onRetry }: { onNew: () => void; onRetry: () => void }): JSX.Element {
  return (
    <div role="alert" class={styles.card}>
      <WidgetHeader icon="fas fa-triangle-exclamation" title="Conversation not found" typeBadge="Bridge" />
      <WidgetBody>
        <div class="widget-message">
          This conversation was not found on the Bridge server. Your saved conversation is still linked to this tab.
        </div>
      </WidgetBody>
      <WidgetActions>
        <WidgetButton variant="accept" onClick={onNew} label="Start new conversation" />
        <WidgetButton variant="dismiss" onClick={onRetry} label="Retry" />
      </WidgetActions>
    </div>
  )
}
