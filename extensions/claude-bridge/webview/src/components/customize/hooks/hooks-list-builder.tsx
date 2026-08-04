// Group/sort/filter helpers for the ActiveHooks list column.
// Extracted from ActiveHooks.tsx (Sprint 5 / Stage E1).

import type { JSX } from 'preact'
import { prettifyPluginName } from '../../../utils/prettify-plugin-name'
import { EventChip, TypeChip } from './HookChips'
import type { FlatHook } from './HookDetail'

export function hookKey(fh: FlatHook, idx: number): string {
  return `${fh.event}:${fh.matcher ?? ''}:${idx}`
}

export function filterHooks(flat: FlatHook[], filter: string): FlatHook[] {
  const q = filter.toLowerCase()
  if (!q) return flat
  return flat.filter(fh => {
    const summary = (fh.handler.command || fh.handler.prompt || fh.handler.url || '').toLowerCase()
    return fh.event.toLowerCase().includes(q)
      || (fh.matcher ?? '').toLowerCase().includes(q)
      || fh.source.kind.toLowerCase().includes(q)
      || summary.includes(q)
  })
}

interface GroupedItem { fh: FlatHook; idx: number }
interface Group { title: string; order: number; items: GroupedItem[] }

export function groupHooks(filtered: FlatHook[]): Array<[string, Group]> {
  const groups: Record<string, Group> = {}
  filtered.forEach((fh, idx) => {
    let key: string, title: string, order: number
    if (fh.source.kind === 'plugin' && fh.source.pluginId) {
      key = `plugin:${fh.source.pluginId}`
      title = prettifyPluginName(fh.source.pluginId.split('@')[0] || fh.source.pluginId)
      order = 100
    } else if (fh.source.kind === 'project') {
      key = 'project'; title = 'Project hooks'; order = 10
    } else if (fh.source.kind === 'user') {
      key = 'user'; title = 'User hooks'; order = 0
    } else if (fh.source.kind === 'bridge') {
      key = 'bridge'; title = 'Bridge built-ins'; order = 200
    } else if (fh.source.kind === 'library') {
      key = 'library'; title = 'Library templates'; order = 300
    } else {
      key = fh.source.kind || 'other'; title = fh.source.kind || 'Other'; order = 400
    }
    if (!groups[key]) groups[key] = { title, order, items: [] }
    groups[key].items.push({ fh, idx })
  })
  return Object.entries(groups).sort(([, a], [, b]) =>
    a.order !== b.order ? a.order - b.order : a.title.localeCompare(b.title)
  )
}

interface RowProps {
  fh: FlatHook
  selected: boolean
  onSelect: () => void
}

export function HookListRow({ fh, selected, onSelect }: RowProps): JSX.Element {
  const summary = fh.handler.command || fh.handler.prompt || fh.handler.url || '(no body)'
  return (
    <button
      type="button"
      onClick={onSelect}
      style={`
        width:100%;text-align:left;display:flex;flex-direction:column;gap:4px;
        padding:8px 12px;border:none;cursor:pointer;
        background:${selected ? 'var(--bg-surface)' : 'transparent'};
        border-left:3px solid ${selected ? 'var(--accent-primary)' : 'transparent'};
        transition:background var(--transition-fast);
      `}
      onMouseEnter={(e: any) => { if (!selected) e.currentTarget.style.background = 'var(--tint-muted-soft)' }}
      onMouseLeave={(e: any) => { if (!selected) e.currentTarget.style.background = 'transparent' }}
    >
      <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap">
        <EventChip event={fh.event} />
        <TypeChip type={fh.handler.type} />
      </div>
      <div style="font-size:var(--fs-sm);color:var(--text-primary);font-family:var(--font-mono);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;width:100%">
        {fh.matcher && <span style="color:var(--accent-yellow);margin-right:6px">{fh.matcher}</span>}
        {summary.length > 70 ? summary.slice(0, 68) + '…' : summary}
      </div>
    </button>
  )
}
