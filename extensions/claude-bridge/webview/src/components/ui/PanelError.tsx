import type { JSX } from 'preact'

/** Compact centered error + Retry for a panel whose data fetch failed. Without
 *  it a failed load falls through to the empty state ("None found"), which
 *  wrongly reads as "nothing here" rather than "couldn't load". */
export function PanelError({ message, onRetry }: { message?: string; onRetry: () => void }): JSX.Element {
  return (
    <div role="alert" style="display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;padding:28px 20px;text-align:center;color:var(--text-muted)">
      <i class="fas fa-triangle-exclamation" style="font-size:22px;color:var(--accent-yellow);opacity:0.85" />
      <div style="font-size:var(--fs-md);font-weight:600;color:var(--text-primary)">Couldn't load</div>
      {message && <div style="font-size:var(--fs-sm);max-width:280px;line-height:1.4">{message}</div>}
      <button
        type="button"
        onClick={onRetry}
        style="display:inline-flex;align-items:center;gap:6px;padding:6px 16px;border-radius:var(--radius-sm);border:1px solid var(--divider-soft, color-mix(in srgb, var(--text-primary) 14%, transparent));background:color-mix(in srgb, var(--text-primary) 6%, transparent);color:var(--text-primary);font-size:var(--fs-sm);cursor:pointer"
      >
        <i class="fas fa-rotate" /> Retry
      </button>
    </div>
  )
}
