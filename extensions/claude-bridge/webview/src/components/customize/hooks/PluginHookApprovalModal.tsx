import type { JSX } from 'preact'
import { useEffect, useRef, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { showToast } from '../../../signals/toasts'
import {
  approvalSelectionKey,
  formatHookDeclaration,
  formatReviewText,
  handlerTypeLabel,
  initiallySelectedHookHashes,
  isReviewableHandler,
  looksDangerous,
  type ApprovalHandler,
} from './hook-approval-display'

interface PendingApproval {
  pluginId: string
  hooks: Array<{ event: string; matcher?: string; handler: ApprovalHandler; hash: string }>
  approvedHashes?: string[]
}

interface SelectionState {
  approvalKey: string
  hashes: Set<string>
}

const EMPTY_SELECTION = new Set<string>()

function showRestartNotice(pluginId: string): void {
  showToast({
    type: 'info',
    title: `${pluginId}: restart required`,
    message: 'Hook approvals are synced. Close and reopen existing chats to apply the new hook set.',
    duration: 12_000,
  })
}

/** Modal that pops when a freshly installed plugin declares hooks. User
 *  must explicitly approve each (or all) before they can fire. Approved
 *  set is stored as sha256 hashes per plugin — re-install of the same
 *  manifest auto-approves; any change re-prompts. */
export function PluginHookApprovalModal(): JSX.Element | null {
  const bridge = useBridge()
  const [queue, setQueue] = useState<PendingApproval[]>([])
  const pending = queue[0] ?? null
  const [selection, setSelection] = useState<SelectionState>({ approvalKey: '', hashes: new Set() })
  const [saving, setSaving] = useState(false)
  const dialogRef = useRef<HTMLDivElement>(null)
  const previousFocusRef = useRef<HTMLElement | null>(null)
  const pendingApprovalKey = pending
    ? approvalSelectionKey(pending.pluginId, pending.hooks, pending.approvedHashes ?? [])
    : ''
  const selected = selection.approvalKey === pendingApprovalKey ? selection.hashes : EMPTY_SELECTION

  useEffect(() => {
    const enqueue = (data: PendingApproval) => {
      setQueue(current => [...current.filter(item => item.pluginId !== data.pluginId), data])
    }
    const off = bridge.onPluginHooksAwaitingApproval(enqueue)
    void bridge.hooksListPendingPluginApprovals().then((items) => {
      setQueue(current => {
        const refreshed = new Set(items.map(item => item.pluginId))
        return [...current.filter(item => !refreshed.has(item.pluginId)), ...items]
      })
    }).catch(() => { /* keep runtime event fallback */ })
    return () => { try { off?.() } catch { /* ignore */ } }
  }, [])

  useEffect(() => {
    if (!pending) return
    previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    // A quiet heuristic is not proof of safety. Only an already reviewed,
    // byte-identical declaration may start selected.
    setSelection({
      approvalKey: pendingApprovalKey,
      hashes: initiallySelectedHookHashes(pending.hooks, pending.approvedHashes ?? []),
    })
    const frame = requestAnimationFrame(() => dialogRef.current?.focus())
    return () => {
      cancelAnimationFrame(frame)
      previousFocusRef.current?.focus()
      previousFocusRef.current = null
    }
  }, [pendingApprovalKey])

  if (!pending) return null

  async function approveSelected(): Promise<void> {
    if (!pending) return
    setSaving(true)
    try {
      const result = await bridge.hooksSetPluginApproval(pending.pluginId, Array.from(selected))
      if (!result?.ok) throw new Error(result?.error || 'Approval could not be saved')
      if (result.restartRequired) showRestartNotice(pending.pluginId)
      setQueue(current => current.slice(1))
    } catch (err) {
      showToast({ type: 'error', title: pending.pluginId, message: err instanceof Error ? err.message : String(err) })
    } finally {
      setSaving(false)
    }
  }

  async function rejectAll(): Promise<void> {
    if (!pending) return
    setSaving(true)
    try {
      const result = await bridge.hooksSetPluginApproval(pending.pluginId, [])
      if (!result?.ok) throw new Error(result?.error || 'Rejection could not be saved')
      if (result.restartRequired) showRestartNotice(pending.pluginId)
      setQueue(current => current.slice(1))
    } catch (err) {
      showToast({ type: 'error', title: pending.pluginId, message: err instanceof Error ? err.message : String(err) })
    } finally {
      setSaving(false)
    }
  }

  function handleDialogKeyDown(event: KeyboardEvent): void {
    if (event.key !== 'Tab' || !dialogRef.current) return
    const focusable = Array.from(dialogRef.current.querySelectorAll<HTMLElement>(
      'input:not([disabled]),button:not([disabled]),[href],[tabindex]:not([tabindex="-1"])',
    )).filter(element => element.offsetParent !== null)
    if (focusable.length === 0) {
      event.preventDefault()
      dialogRef.current.focus()
      return
    }
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    const active = document.activeElement
    if (active === dialogRef.current || !dialogRef.current.contains(active)) {
      event.preventDefault()
      if (event.shiftKey) last.focus(); else first.focus()
    } else if (event.shiftKey && active === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && active === last) {
      event.preventDefault()
      first.focus()
    }
  }

  async function copyDeclaration(handler: ApprovalHandler): Promise<void> {
    try {
      await navigator.clipboard.writeText(formatHookDeclaration(handler))
      showToast({ type: 'success', title: pending.pluginId, message: 'Hook declaration copied' })
    } catch (err) {
      showToast({ type: 'error', title: pending.pluginId, message: err instanceof Error ? err.message : String(err) })
    }
  }

  return (
    <div style="
      position:fixed;inset:0;background:var(--overlay-deep);
      display:flex;align-items:center;justify-content:center;
      z-index:var(--z-modal);padding:var(--space-4);box-sizing:border-box;
    ">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="plugin-hook-approval-title"
        aria-describedby="plugin-hook-approval-description"
        tabIndex={-1}
        onKeyDown={handleDialogKeyDown}
        style="
          background:var(--bg-mantle);border-radius:var(--radius-lg);
          padding:var(--space-5);width:min(780px,100%);max-height:calc(100vh - 2 * var(--space-4));
          overflow-y:auto;overflow-x:hidden;box-shadow:var(--shadow-lg);box-sizing:border-box;
        "
      >
        <h2 id="plugin-hook-approval-title" style="margin:0 0 var(--space-2);font-size:var(--fs-xl);color:var(--text-primary);overflow-wrap:anywhere">
          Review hooks from <code style="color:var(--accent-purple)">{pending.pluginId}</code>
        </h2>
        <p id="plugin-hook-approval-description" style="margin:0 0 var(--space-4);color:var(--text-secondary);font-size:var(--fs-sm);line-height:var(--lh-base)">
          This plugin defines {pending.hooks.length} hook{pending.hooks.length === 1 ? '' : 's'} that
          can run automatically when CLI events fire. Review each one and approve only what you trust.
          Hooks marked <span style="color:var(--accent-red)">⚠ dangerous</span> contain commands like
          <code> rm -rf</code>, <code>sudo</code>, <code>curl | sh</code> — handle with extreme care.
          An unmarked hook is not automatically safe and remains unselected until you approve it.
        </p>

        <div style="display:flex;flex-direction:column;gap:var(--space-2);margin-bottom:var(--space-4)">
          {pending.hooks.map(h => {
            const danger = looksDangerous(h.handler)
            const reviewable = isReviewableHandler(h.handler)
            const declaration = formatHookDeclaration(h.handler)
            const matcher = h.matcher ? formatReviewText(h.matcher) : ''
            const checkboxId = `plugin-hook-${h.hash}`
            return (
              <div
                key={h.hash}
                style={`
                  display:flex;gap:var(--space-2);align-items:flex-start;
                  padding:var(--space-3);border-radius:var(--radius-sm);
                  background:var(--bg-base);border-left:3px solid ${danger || !reviewable ? 'var(--accent-red)' : 'var(--accent-primary)'};
                  min-width:0;
                `}
              >
                <input
                  id={checkboxId}
                  type="checkbox"
                  checked={selected.has(h.hash)}
                  disabled={saving || !reviewable}
                  onChange={(e: any) => {
                    const next = new Set(selected)
                    if (e.target.checked) next.add(h.hash); else next.delete(h.hash)
                    setSelection({ approvalKey: pendingApprovalKey, hashes: next })
                  }}
                  style="margin-top:4px;flex-shrink:0"
                />
                <div style="flex:1;min-width:0">
                  <div style="display:flex;gap:var(--space-2);align-items:center;flex-wrap:wrap;margin-bottom:4px">
                    <label for={checkboxId} style="cursor:pointer;display:flex;gap:var(--space-2);align-items:center;flex-wrap:wrap;min-width:0">
                      <span style="font-size:var(--fs-xs);padding:1px 8px;border-radius:999px;background:var(--tint-primary-strong);color:var(--accent-primary);font-weight:700">
                        {h.event}
                      </span>
                    </label>
                    <span style="font-size:var(--fs-xs);color:var(--text-muted)">{handlerTypeLabel(h.handler)}</span>
                    {!reviewable && <span style="font-size:var(--fs-xs);color:var(--accent-red);font-weight:700">invalid declaration — cannot approve</span>}
                    {danger && <span style="font-size:var(--fs-xs);color:var(--accent-red);font-weight:700">⚠ dangerous pattern</span>}
                  </div>
                  {matcher && (
                    <div style="margin:0 0 6px;font-size:var(--fs-xs);color:var(--accent-yellow);font-family:var(--font-mono);white-space:pre-wrap;overflow-wrap:anywhere;max-height:72px;overflow:auto" title={matcher}>
                      <span style="color:var(--text-muted);font-family:inherit">matcher: </span>{matcher}
                    </div>
                  )}
                  <pre style="
                    margin:0;display:block;font-size:var(--fs-sm);color:var(--text-primary);
                    font-family:var(--font-mono);white-space:pre-wrap;word-break:break-word;
                    background:var(--overlay-soft);padding:6px 8px;border-radius:var(--radius-sm);
                    max-height:220px;overflow:auto;user-select:text;
                  ">
                    {declaration}
                  </pre>
                  <div style="display:flex;justify-content:flex-end;margin-top:6px">
                    <button
                      type="button"
                      onClick={() => { void copyDeclaration(h.handler) }}
                      disabled={saving}
                      style="padding:4px 8px;background:transparent;border:1px solid var(--bg-surface);color:var(--text-secondary);border-radius:var(--radius-sm);cursor:pointer;font-size:var(--fs-xs)"
                    >
                      Copy declaration
                    </button>
                  </div>
                </div>
              </div>
            )
          })}
        </div>

        <div style="display:flex;gap:var(--space-2);justify-content:flex-end;flex-wrap:wrap">
          <button
            type="button"
            onClick={rejectAll}
            disabled={saving}
            style="padding:8px 16px;background:transparent;border:1px solid var(--accent-red);color:var(--accent-red);border-radius:var(--radius-sm);cursor:pointer;font-weight:600"
          >
            Reject all
          </button>
          <button
            type="button"
            onClick={() => setSelection({
              approvalKey: pendingApprovalKey,
              hashes: new Set(pending.hooks.filter(h => isReviewableHandler(h.handler)).map(h => h.hash)),
            })}
            disabled={saving}
            style="padding:8px 16px;background:transparent;border:1px solid var(--bg-surface);color:var(--text-secondary);border-radius:var(--radius-sm);cursor:pointer"
          >
            Select all
          </button>
          <button
            type="button"
            onClick={approveSelected}
            disabled={saving || selected.size === 0}
            style="padding:8px 16px;background:var(--accent-green);color:var(--bg-mantle);border:none;border-radius:var(--radius-sm);cursor:pointer;font-weight:600"
          >
            {saving ? 'Saving…' : `Approve ${selected.size} of ${pending.hooks.length}`}
          </button>
        </div>
      </div>
    </div>
  )
}
