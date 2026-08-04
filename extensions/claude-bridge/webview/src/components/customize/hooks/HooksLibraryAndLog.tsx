// Library tab + Log tab + LogRow row component — extracted from
// `HooksPanel.tsx` (Sprint 2 / Stage C, C10 second pass). Both tabs are
// self-contained: each has its own state, fetches its own data via the
// bridge, and only depends on shared chips + types.

import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { EmptyState } from '../../ui/EmptyState'
import { ListSkeleton } from '../../ui/ListSkeleton'
import { PanelError } from '../../ui/PanelError'
import { showToast } from '../../../signals/toasts'
import { EventChip, HostChip, SourceChip, CategoryChip } from './HookChips'

interface HookHandler {
  type: string
  command?: string
  prompt?: string
  url?: string
  host?: string
  if?: string
  timeout?: number
  shell?: string
}
interface HookMatcher {
  matcher?: string
  hooks: HookHandler[]
}
type HooksByEvent = Record<string, HookMatcher[]>

interface LibraryTemplate {
  id: string
  title: string
  description: string
  event: string
  matcher?: string
  handler: HookHandler
  rationale: string
  category: string
  recommendedHost?: string
}

interface LogEntry {
  ts: number
  sessionId: string
  hookId: string
  event: string
  matcher?: string
  source: { kind: string; pluginId?: string; projectPath?: string }
  effectiveHost: string
  outcome: string
  exitCode: number
  durationMs: number
  stdoutPreview?: string
  stderrPreview?: string
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!))
}
async function customConfirm(title: string, message: string, opts?: { confirmLabel?: string; isDanger?: boolean }): Promise<boolean> {
  const show = (window as any).__showConfirmModal as ((o: { title: string; bodyHtml: string; confirmLabel?: string; isDanger?: boolean }) => Promise<boolean>) | undefined
  if (typeof show === 'function') {
    const result = await show({ title, bodyHtml: `<p>${escapeHtml(message)}</p>`, confirmLabel: opts?.confirmLabel ?? 'Delete', isDanger: opts?.isDanger ?? true })
    return !!result
  }
  return false
}

// ─── Library tab ────────────────────────────────────────────────────────────

export function LibraryHooks(): JSX.Element {
  const bridge = useBridge()
  const [templates, setTemplates] = useState<LibraryTemplate[]>([])
  const [installing, setInstalling] = useState<string | null>(null)
  const [installed, setInstalled] = useState<Set<string>>(new Set())
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)

  // Detect "already installed": user has a hook entry in their settings.json
  // matching this template's event+matcher+(handler shape). Hash-compare on
  // command/prompt/url + matcher gets us 99% accuracy without a strict deep eq.
  function templateInstalled(tmpl: LibraryTemplate, hooks: HooksByEvent): boolean {
    const matchers = hooks[tmpl.event] ?? []
    for (const m of matchers) {
      if ((m.matcher ?? '') !== (tmpl.matcher ?? '')) continue
      for (const h of m.hooks) {
        if (h.type !== tmpl.handler.type) continue
        if (h.command && h.command === tmpl.handler.command) return true
        if (h.prompt && h.prompt === tmpl.handler.prompt) return true
        if (h.url && h.url === tmpl.handler.url) return true
      }
    }
    return false
  }

  async function refreshAll(): Promise<void> {
    setLoadError(false)
    try {
      const [libRes, listRes] = await Promise.all([
        bridge.hooksGetLibrary(),
        bridge.hooksList(),
      ])
      const tmpls = (libRes?.templates ?? []) as unknown as LibraryTemplate[]
      const hooks = (listRes?.hooks ?? {}) as unknown as HooksByEvent
      setTemplates(tmpls)
      // Pre-populate `installed` set from existing user-settings so the button
      // shows "✓ Installed" on a template that was added in a prior session.
      const already = new Set<string>()
      for (const t of tmpls) {
        if (templateInstalled(t, hooks)) already.add(t.id)
      }
      setInstalled(already)
    } catch {
      setLoadError(true)
    } finally {
      setLoading(false)
    }
  }
  useEffect(() => { void refreshAll() }, [])

  if (loading && templates.length === 0) {
    return <ListSkeleton rows={5} />
  }
  if (loadError && templates.length === 0) {
    return <PanelError message="Could not load hook templates." onRetry={() => { setLoading(true); void refreshAll() }} />
  }

  async function install(id: string): Promise<void> {
    setInstalling(id)
    try {
      const r = await bridge.hooksInstallTemplate(id)
      if (r?.ok) {
        setInstalled(prev => new Set(prev).add(id))
        showToast({ type: 'success', title: 'Hook installed', message: 'Visit the Active tab to verify.', duration: 4000 })
      } else {
        showToast({ type: 'error', title: 'Install failed', message: r?.error ?? 'unknown error', duration: 5000 })
      }
    } finally {
      setInstalling(null)
    }
  }

  if (templates.length === 0) {
    return (
      <EmptyState
        icon="fa-box-archive"
        title="No templates available."
        hint="The bridge ships a curated set of CLAUDE.md / lint / format hooks. Restart the app if this list stays empty."
      />
    )
  }

  return (
    <div style="flex:1;overflow-y:auto;padding:var(--space-3) var(--space-4)">
      <div style="display:flex;flex-direction:column;gap:var(--space-3)">
        {templates.map(t => (
          <div key={t.id} style="background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-md);padding:var(--space-3);display:flex;flex-direction:column;gap:6px">
            <div style="display:flex;align-items:flex-start;gap:var(--space-2);flex-wrap:wrap">
              <div style="flex:1;min-width:0">
                <div style="font-weight:600;color:var(--text-primary);font-size:var(--fs-md)">{t.title}</div>
                <div style="font-size:var(--fs-sm);color:var(--text-secondary);margin-top:2px">{t.description}</div>
              </div>
              <CategoryChip category={t.category} />
            </div>
            <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap">
              <EventChip event={t.event} />
              {t.matcher && <span style="font-size:var(--fs-xs);padding:1px 6px;border-radius:999px;background:var(--tint-yellow-medium);color:var(--accent-yellow);font-family:var(--font-mono)">{t.matcher}</span>}
              <HostChip host={(t.recommendedHost ?? 'local') as any} />
            </div>
            {t.rationale && <div style="font-size:var(--fs-xs);color:var(--text-muted);font-style:italic">— {t.rationale}</div>}
            <button
              type="button"
              onClick={() => install(t.id)}
              disabled={installed.has(t.id) || installing === t.id}
              style={`padding:6px 14px;align-self:flex-start;background:${installed.has(t.id) ? 'var(--bg-surface)' : 'var(--accent-primary)'};color:${installed.has(t.id) ? 'var(--text-secondary)' : 'var(--bg-mantle)'};border:none;border-radius:var(--radius-sm);cursor:${installed.has(t.id) ? 'default' : 'pointer'};font-size:var(--fs-sm);font-weight:600`}
            >
              {installed.has(t.id) ? '✓ Installed' : installing === t.id ? 'Installing…' : 'Install'}
            </button>
          </div>
        ))}
      </div>
    </div>
  )
}

// ─── Log tab ────────────────────────────────────────────────────────────────

export function LogPane(): JSX.Element {
  const bridge = useBridge()
  const [entries, setEntries] = useState<LogEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)

  async function refresh(): Promise<void> {
    setLoading(true)
    setLoadError(false)
    try {
      const r = await bridge.hooksGetLog(200)
      setEntries(r?.entries ?? [])
    } catch {
      setLoadError(true)
    } finally {
      setLoading(false)
    }
  }
  useEffect(() => { void refresh() }, [])

  if (loading && entries.length === 0) {
    return <ListSkeleton rows={6} />
  }
  if (loadError && entries.length === 0) {
    return <PanelError message="Could not load the hook log." onRetry={() => { void refresh() }} />
  }
  if (!loading && entries.length === 0) {
    return (
      <EmptyState
        icon="fa-list-timeline"
        title="No hook executions yet."
        hint="Configure a hook in the Active or Library tab, then trigger its event in chat."
      />
    )
  }
  return (
    <div style="flex:1;display:flex;flex-direction:column;min-height:0">
      <div style="display:flex;align-items:center;gap:var(--space-2);padding:var(--space-2) var(--space-3);border-bottom:1px solid var(--bg-surface);font-size:var(--fs-sm);color:var(--text-muted);flex-shrink:0">
        <span>{entries.length} entries</span>
        <button
          type="button"
          onClick={refresh}
          style="background:transparent;border:none;color:var(--accent-primary);cursor:pointer;padding:2px 6px"
        >
          <i class="fas fa-rotate" /> Refresh
        </button>
        <button
          type="button"
          onClick={async () => {
            const ok = await customConfirm('Clear hook log?', 'Erase the entire hook execution log? This cannot be undone.', { confirmLabel: 'Clear', isDanger: true })
            if (!ok) return
            await bridge.hooksClearLog()
            refresh()
          }}
          style="margin-left:auto;background:transparent;border:none;color:var(--accent-red);cursor:pointer;padding:2px 6px"
        >
          <i class="fas fa-trash" /> Clear
        </button>
      </div>
      <div style="flex:1;overflow-y:auto">
        {entries.map(e => <LogRow key={`${e.ts}-${e.hookId}`} entry={e} />)}
      </div>
    </div>
  )
}

function LogRow({ entry }: { entry: LogEntry }): JSX.Element {
  const outcomeColor: Record<string, string> = {
    success: 'var(--accent-green)',
    error: 'var(--accent-red)',
    timeout: 'var(--accent-yellow)',
    cancelled: 'var(--text-muted)',
  }
  const c = outcomeColor[entry.outcome] ?? 'var(--text-muted)'
  return (
    <div style="display:grid;grid-template-columns:140px 110px 110px 70px 60px 1fr;gap:var(--space-2);padding:var(--space-2) var(--space-3);
      border-bottom:1px solid var(--tint-surface-soft);align-items:center;
      font-size:var(--fs-sm);font-family:var(--font-mono)">
      <span style="color:var(--text-muted)">{new Date(entry.ts).toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })}</span>
      <EventChip event={entry.event} />
      <SourceChip source={entry.source} />
      <span style={`color:${c};font-weight:700`}>{entry.outcome}</span>
      <span style={`color:${c}`}>exit {entry.exitCode}</span>
      <span style="color:var(--text-secondary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
        {entry.matcher || '-'} · {entry.durationMs}ms
        {entry.stderrPreview && <span style="color:var(--accent-red);margin-left:8px" title={entry.stderrPreview}>{entry.stderrPreview.slice(0, 60)}</span>}
      </span>
    </div>
  )
}
