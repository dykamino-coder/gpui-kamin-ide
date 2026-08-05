// Active hooks tab — 3-column list/detail of currently-installed hooks.
// Owns: data fetch, filter, selection, editor modal. Detail card and
// list-row rendering are in sibling modules to keep this thin.

import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { readPanelCache, writePanelCache } from '../../../lib/panel-cache'
import { EmptyState } from '../../ui/EmptyState'
import { PanelError } from '../../ui/PanelError'
import { HookEditor } from './HookEditor'
import { HookTestRunner } from './HookTestRunner'
import { ThreeColumnPanel } from '../shared/ThreeColumnPanel'
import { ThreeColumnList } from '../shared/ThreeColumnList'
import { ThreeColumnDetail } from '../shared/ThreeColumnDetail'
import { ThreeColumnListFooter } from '../shared/ThreeColumnListFooter'
import { showToast } from '../../../signals/toasts'
import { HookDetail } from './HookDetail'
import type { FlatHook } from './HookDetail'
import { hookKey, filterHooks, groupHooks, HookListRow } from './hooks-list-builder'

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

export function ActiveHooks(): JSX.Element {
  const bridge = useBridge()
  const [flatHooks, setFlatHooks] = useState<FlatHook[]>([])
  const [userHooks, setUserHooks] = useState<HooksByEvent>({})
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)
  const [editor, setEditor] = useState<{ event: string; matcher?: string; handler: any; index?: { matcherIdx: number; handlerIdx: number } } | null>(null)
  const [showTest, setShowTest] = useState(false)
  const [filter, setFilter] = useState('')
  const [selectedKey, setSelectedKey] = useState<string | null>(null)

  // `showSpinner=false` on the cache-first revalidation path so we don't flip
  // the "Loading…" row back on over already-painted cached data.
  async function refresh(showSpinner = true): Promise<void> {
    if (showSpinner) setLoading(true)
    setLoadError(false)
    try {
      const [activeTabId, tabs, settingsList] = await Promise.all([
        bridge.getActiveTab(),
        bridge.listTabs(),
        bridge.hooksList(),
      ])
      const sessionId = tabs.find(tab => tab.id === activeTabId)?.sessionId
      const active = sessionId ? await bridge.hooksActive(sessionId) : { hooks: [] }
      const flat = (active?.hooks ?? []) as unknown as FlatHook[]
      const user = (settingsList?.hooks ?? {}) as unknown as HooksByEvent
      setFlatHooks(flat)
      setUserHooks(user)
      writePanelCache('hooks.active.v1', { flat, user })
    } catch {
      // Surface the failure so a broken load reads as "couldn't load" (PanelError
      // + Retry) instead of silently falling through to the "No hooks" empty state.
      setLoadError(true)
    } finally {
      setLoading(false)
    }
  }
  useEffect(() => {
    // Cache-first: paint the last-known hooks instantly (clears the spinner),
    // then revalidate in the background WITHOUT re-showing the spinner.
    const cached = readPanelCache<{ flat: FlatHook[]; user: HooksByEvent }>('hooks.active.v1')
    if (cached) {
      setFlatHooks(cached.flat)
      setUserHooks(cached.user)
      setLoading(false)
    }
    void refresh(!cached).catch(() => {})
  }, [])

  function findUserIndex(h: FlatHook): { matcherIdx: number; handlerIdx: number } | null {
    if (h.source.kind !== 'user') return null
    const matchers = userHooks[h.event] ?? []
    for (let mi = 0; mi < matchers.length; mi++) {
      const m = matchers[mi]!
      for (let hi = 0; hi < m.hooks.length; hi++) {
        if (m.hooks[hi] === h.handler || (m.matcher === h.matcher && JSON.stringify(m.hooks[hi]) === JSON.stringify(h.handler))) {
          return { matcherIdx: mi, handlerIdx: hi }
        }
      }
    }
    return null
  }

  const filtered = filterHooks(flatHooks, filter)
  const selected = filtered.find((fh, idx) => hookKey(fh, idx) === selectedKey) ?? null
  const sortedGroups = groupHooks(filtered)

  return (
    <ThreeColumnPanel>
      <ThreeColumnList
        title="Hooks"
        searchValue={filter}
        onSearchChange={setFilter}
        searchPlaceholder="Search by event, matcher, source, or command…"
        footer={
          <ThreeColumnListFooter
            label="New hook"
            onClick={() => setEditor({ event: 'PostToolUse', handler: { type: 'command', command: '', shell: 'bash', host: 'auto', timeout: 30 } })}
          />
        }
      >
        {loading && <div style="padding:var(--space-3);color:var(--text-muted);font-size:var(--fs-sm)">Loading…</div>}
        {!loading && loadError && flatHooks.length === 0 && (
          <PanelError message="Could not load hooks." onRetry={() => { setLoading(true); void refresh() }} />
        )}
        {!loading && !loadError && filtered.length === 0 && (
          <EmptyState
            icon="fa-link"
            title={flatHooks.length === 0 ? 'No hooks configured.' : 'No hooks match the filter.'}
            hint={flatHooks.length === 0
              ? <>Click <b>+ New hook</b>, install a template from the <b>Library</b> tab, or edit <code>~/.claude/settings.json</code> directly.</>
              : 'Adjust the search query.'}
          />
        )}
        {sortedGroups.map(([gkey, g]) => (
          <div key={gkey}>
            <div style="font-size:var(--fs-xs);text-transform:uppercase;letter-spacing:0.5px;color:var(--text-disabled);padding:12px 16px 4px">
              {g.title}
            </div>
            {g.items.map(({ fh, idx }) => {
              const key = hookKey(fh, idx)
              return (
                <HookListRow
                  key={key}
                  fh={fh}
                  selected={key === selectedKey}
                  onSelect={() => { setSelectedKey(key); setShowTest(false) }}
                />
              )
            })}
          </div>
        ))}
      </ThreeColumnList>

      <ThreeColumnDetail>
        {selected ? (
          <>
            <HookDetail
              fh={selected}
              editable={!!findUserIndex(selected)}
              showTest={showTest}
              onToggleTest={() => setShowTest(s => !s)}
              onEdit={() => {
                const idx = findUserIndex(selected)
                if (idx) setEditor({ event: selected.event, matcher: selected.matcher, handler: selected.handler, index: idx })
              }}
              onDelete={async () => {
                const idx = findUserIndex(selected)
                if (!idx) {
                  showToast({
                    type: 'info',
                    title: 'Read-only hook',
                    message: `Comes from ${selected.source.kind}${selected.source.pluginId ? `: ${selected.source.pluginId}` : ''}. Edit its source file directly.`,
                    duration: 5000,
                  })
                  return
                }
                const ok = await customConfirm(
                  'Delete hook?',
                  `Delete this ${selected.event} hook from your user settings? This cannot be undone.`,
                  { confirmLabel: 'Delete', isDanger: true },
                )
                if (!ok) return
                await bridge.hooksDelete(selected.event, idx.matcherIdx, idx.handlerIdx)
                setSelectedKey(null)
                refresh()
              }}
            />
            {showTest && (
              <div style="margin-top:var(--space-3);padding-top:var(--space-3);border-top:1px solid var(--bg-surface)">
                <HookTestRunner
                  event={selected.event}
                  matcher={selected.matcher}
                  handler={selected.handler}
                />
              </div>
            )}
          </>
        ) : (
          <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;height:100%;color:var(--text-muted);text-align:center;padding:var(--space-5)">
            <i class="fas fa-link" style="font-size:32px;opacity:0.3;margin-bottom:var(--space-3)" />
            <div style="font-size:var(--fs-md);margin-bottom:var(--space-2);color:var(--text-secondary)">Select a hook to inspect</div>
            <div style="font-size:var(--fs-sm);max-width:380px;line-height:var(--lh-base)">
              Pick any hook from the list to see its event, source, runtime location, command and recent executions.
            </div>
          </div>
        )}
      </ThreeColumnDetail>

      {editor && (
        <HookEditor
          initial={editor}
          onSave={() => { setEditor(null); refresh() }}
          onCancel={() => setEditor(null)}
        />
      )}
    </ThreeColumnPanel>
  )
}
