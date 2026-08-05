import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { showToast } from '../../../signals/toasts'

interface PendingApproval {
  pluginId: string
  hooks: Array<{ event: string; matcher?: string; handler: any; hash: string }>
  approvedHashes?: string[]
}

const DANGEROUS_PATTERNS: RegExp[] = [
  /rm\s+-rf\s+/,
  /sudo\s+/,
  /curl[^|]*\|\s*sh/,
  /eval\s+/,
  /\bdd\s+if=.*of=\/dev\//,
  />\s*\/dev\/sda/,
  /\bnc\s+-l/,
  /:\(\)\s*\{\s*:\s*\|\s*:\s*&\s*\}\s*;:/,
]

function looksDangerous(handler: any): boolean {
  const text = handler.command || handler.prompt || handler.url || ''
  return DANGEROUS_PATTERNS.some(p => p.test(text))
}

/** Modal that pops when a freshly installed plugin declares hooks. User
 *  must explicitly approve each (or all) before they can fire. Approved
 *  set is stored as sha256 hashes per plugin — re-install of the same
 *  manifest auto-approves; any change re-prompts. */
export function PluginHookApprovalModal(): JSX.Element | null {
  const bridge = useBridge()
  const [queue, setQueue] = useState<PendingApproval[]>([])
  const pending = queue[0] ?? null
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [saving, setSaving] = useState(false)

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
    if (pending) {
      // Pre-select all non-dangerous hooks plus hashes approved previously.
      const initial = new Set<string>()
      const alreadyApproved = new Set(pending.approvedHashes ?? [])
      for (const h of pending.hooks) if (alreadyApproved.has(h.hash) || !looksDangerous(h.handler)) initial.add(h.hash)
      setSelected(initial)
    }
  }, [pending])

  if (!pending) return null

  async function approveSelected(): Promise<void> {
    if (!pending) return
    setSaving(true)
    try {
      const result = await bridge.hooksSetPluginApproval(pending.pluginId, Array.from(selected))
      if (!result?.ok) throw new Error(result?.error || 'Approval could not be saved')
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
      setQueue(current => current.slice(1))
    } catch (err) {
      showToast({ type: 'error', title: pending.pluginId, message: err instanceof Error ? err.message : String(err) })
    } finally {
      setSaving(false)
    }
  }

  return (
    <div style="
      position:fixed;inset:0;background:var(--overlay-deep);
      display:flex;align-items:center;justify-content:center;
      z-index:var(--z-modal);
    ">
      <div style="
        background:var(--bg-mantle);border-radius:var(--radius-lg);
        padding:var(--space-5);min-width:560px;max-width:780px;max-height:85vh;
        overflow-y:auto;box-shadow:var(--shadow-lg);
      ">
        <h2 style="margin:0 0 var(--space-2);font-size:var(--fs-xl);color:var(--text-primary)">
          Review hooks from <code style="color:var(--accent-purple)">{pending.pluginId}</code>
        </h2>
        <p style="margin:0 0 var(--space-4);color:var(--text-secondary);font-size:var(--fs-sm);line-height:var(--lh-base)">
          This plugin defines {pending.hooks.length} hook{pending.hooks.length === 1 ? '' : 's'} that
          can run automatically when CLI events fire. Review each one and approve only what you trust.
          Hooks marked <span style="color:var(--accent-red)">⚠ dangerous</span> contain commands like
          <code> rm -rf</code>, <code>sudo</code>, <code>curl | sh</code> — handle with extreme care.
        </p>

        <div style="display:flex;flex-direction:column;gap:var(--space-2);margin-bottom:var(--space-4)">
          {pending.hooks.map(h => {
            const danger = looksDangerous(h.handler)
            const summary = h.handler.command || h.handler.prompt || h.handler.url || '(no body)'
            return (
              <label
                key={h.hash}
                style={`
                  display:flex;gap:var(--space-2);align-items:flex-start;
                  padding:var(--space-3);border-radius:var(--radius-sm);
                  background:var(--bg-base);border-left:3px solid ${danger ? 'var(--accent-red)' : 'var(--accent-primary)'};
                  cursor:pointer;
                `}
              >
                <input
                  type="checkbox"
                  checked={selected.has(h.hash)}
                  onChange={(e: any) => {
                    const next = new Set(selected)
                    if (e.target.checked) next.add(h.hash); else next.delete(h.hash)
                    setSelected(next)
                  }}
                  style="margin-top:4px;flex-shrink:0"
                />
                <div style="flex:1;min-width:0">
                  <div style="display:flex;gap:var(--space-2);align-items:center;flex-wrap:wrap;margin-bottom:4px">
                    <span style="font-size:var(--fs-xs);padding:1px 8px;border-radius:999px;background:var(--tint-primary-strong);color:var(--accent-primary);font-weight:700">
                      {h.event}
                    </span>
                    {h.matcher && <span style="font-size:var(--fs-xs);color:var(--accent-yellow);font-family:var(--font-mono)">{h.matcher}</span>}
                    <span style="font-size:var(--fs-xs);color:var(--text-muted)">{h.handler.type}</span>
                    {danger && <span style="font-size:var(--fs-xs);color:var(--accent-red);font-weight:700">⚠ dangerous pattern</span>}
                  </div>
                  <code style="
                    display:block;font-size:var(--fs-sm);color:var(--text-primary);
                    font-family:var(--font-mono);white-space:pre-wrap;word-break:break-word;
                    background:var(--overlay-soft);padding:6px 8px;border-radius:var(--radius-sm);
                  ">
                    {summary.length > 400 ? summary.slice(0, 398) + '…' : summary}
                  </code>
                </div>
              </label>
            )
          })}
        </div>

        <div style="display:flex;gap:var(--space-2);justify-content:flex-end">
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
            onClick={() => setSelected(new Set(pending.hooks.map(h => h.hash)))}
            disabled={saving}
            style="padding:8px 16px;background:transparent;border:1px solid var(--bg-surface);color:var(--text-secondary);border-radius:var(--radius-sm);cursor:pointer"
          >
            Select all
          </button>
          <button
            type="button"
            onClick={approveSelected}
            disabled={saving}
            style="padding:8px 16px;background:var(--accent-green);color:var(--bg-mantle);border:none;border-radius:var(--radius-sm);cursor:pointer;font-weight:600"
          >
            {saving ? 'Saving…' : `Approve ${selected.size} of ${pending.hooks.length}`}
          </button>
        </div>
      </div>
    </div>
  )
}
