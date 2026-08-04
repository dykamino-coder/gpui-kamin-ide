// Empty-state main panel. In KaminIDE session creation lives in the HOST
// sidebar — the chat pane must NEVER show its own "Open Claude Bridge / New
// session" welcome (user demand, repeated): with no bound tab it shows the
// restoring cue while init is in flight and a plain quiet surface otherwise.

import type { JSX } from 'preact'
import { LoadingCue } from '../common/LoadingCue'

/** Shown in the main panel while `useInit` is still restoring persisted tabs —
 *  a returning user whose session is still loading sees "Restoring…" instead of
 *  an empty pane, so they don't think the chat is gone. */
export function RestoringContent(): JSX.Element {
  return (
    <div style="flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;color:var(--text-disabled);">
      <LoadingCue title="Restoring your session…" sub="Reopening your saved sessions" />
    </div>
  )
}

/** No bound tab and init finished: a quiet empty surface. No branding, no
 *  buttons — the host sidebar owns session creation. */
export function NoSessionEmptyContent(): JSX.Element {
  return <div style="flex:1" />
}
