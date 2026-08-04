// Extracted from PluginCard.tsx (Sprint 5 / Stage E1) — clone-failure
// banner + Retry button (or muted "not shipped" note for local-source
// failures that aren't retryable).

import type { JSX } from 'preact'
import styles from './PluginCard.module.css'

interface Props {
  isLocalSourceMissing: boolean
  declaredSourcePath?: string
  cloneUrl?: string
  cloneRef?: string
  cloneError?: string
  retrying: boolean
  onRetry: () => void
}

export function PluginCardCloneError({
  isLocalSourceMissing, declaredSourcePath, cloneUrl, cloneRef, cloneError, retrying, onRetry,
}: Props): JSX.Element {
  if (isLocalSourceMissing) {
    return (
      <div class={styles.localMissingNote}>
        <i class="fas fa-ghost" style="margin-right:6px;opacity:0.7" />
        Not shipped with this marketplace clone
        {declaredSourcePath && (
          <span style="margin-left:6px;font-family:var(--font-mono);font-size:10px;color:var(--text-muted)">
            ({declaredSourcePath})
          </span>
        )}
      </div>
    )
  }
  return (
    <>
      <div class={styles.brokenWarning}>
        <i class="fas fa-exclamation-triangle" style="margin-right:6px" />
        Failed to download plugin source{cloneUrl ? `: ${cloneUrl}` : ''}{cloneRef ? ` @ ${cloneRef}` : ''}
        {cloneError && <div style="margin-top:6px;font-size:11px;opacity:0.8;font-family:var(--font-mono);white-space:pre-wrap;word-break:break-word">{cloneError}</div>}
      </div>
      <div class={styles.brokenActions}>
        <button
          class={styles.openMarketplaceBtn}
          onClick={onRetry}
          disabled={retrying}
          title="Re-clone the plugin repository"
        >
          {retrying
            ? <><i class="fas fa-spinner fa-spin" /> Retrying…</>
            : <><i class="fas fa-rotate-right" /> Retry</>}
        </button>
      </div>
    </>
  )
}
