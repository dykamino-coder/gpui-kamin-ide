import type { JSX } from 'preact'
import { useComputed } from '@preact/signals'
import { activeTabId } from '../../signals/tabs'
import { jsonlEntriesByTab, getJsonlStructureVersion } from '../../signals/jsonl'
import type { JsonlEntryData, ContentBlock } from '../../types/jsonl'

function computeStats(entries: JsonlEntryData[]): { pending: number; done: number; total: number } {
  if (entries.length === 0) return { pending: 0, done: 0, total: 0 }
  // Find the last user-authored turn boundary (ignore tool_result user entries).
  let boundary = entries.length - 1
  for (let i = entries.length - 1; i >= 0; i--) {
    const e = entries[i]
    if (e.type !== 'user') continue
    const c = e.message?.content
    if (Array.isArray(c)) {
      const onlyToolResult = (c as ContentBlock[]).every((b) => b.type === 'tool_result')
      if (onlyToolResult) continue
    }
    boundary = i
    break
  }

  const toolUses = new Set<string>()
  const completed = new Set<string>()
  for (let i = boundary; i < entries.length; i++) {
    const e = entries[i]
    const c = e.message?.content
    if (!Array.isArray(c)) continue
    for (const b of c as ContentBlock[]) {
      if (e.type === 'assistant' && b.type === 'tool_use') {
        const id = (b as any).id
        if (typeof id === 'string') toolUses.add(id)
      } else if (e.type === 'user' && b.type === 'tool_result') {
        const id = (b as any).tool_use_id
        if (typeof id === 'string') completed.add(id)
      }
    }
  }
  const total = toolUses.size
  let done = 0
  for (const id of toolUses) if (completed.has(id)) done++
  return { pending: total - done, done, total }
}

// Мемо по структуре: tool_use/tool_result появляются только со структурным
// аппендом; текстовые rAF-дельты стрима счётчики не меняют, а computed
// дёргался на каждый кадр и гнал скан хода (аудит #70 D4).
const toastCache = { key: '', value: { pending: 0, done: 0, total: 0 } }

export function ToolCounterToast(): JSX.Element | null {
  const stats = useComputed(() => {
    const tabId = activeTabId.value
    if (!tabId) return { pending: 0, done: 0, total: 0 }
    const entries = jsonlEntriesByTab.value.get(tabId) ?? []
    const key = `${tabId}|${getJsonlStructureVersion(tabId)}`
    if (toastCache.key === key) return toastCache.value
    const out = computeStats(entries)
    toastCache.key = key
    toastCache.value = out
    return out
  })

  const { pending, done, total } = stats.value
  // Show the toast only while the current turn has tools in flight.
  // Once pending drops to zero, hide immediately — the final turn state is
  // represented by the assistant message itself.
  if (pending <= 0 || total <= 1) return null

  return (
    <div
      style="position:absolute;top:12px;right:20px;z-index:900;display:flex;align-items:center;gap:8px;padding:6px 12px;background:var(--bg-primary);border:1px solid var(--bg-overlay);border-radius:var(--radius-sm);font-size:12px;color:var(--text-primary);box-shadow:var(--shadow-mini);pointer-events:none"
      aria-live="polite"
    >
      <i class="fas fa-bolt" style="color:var(--accent-primary);font-size:11px" />
      <span>
        <span style="color:var(--accent-primary);font-weight:600">{pending}</span> pending · <span style="color:var(--accent-green);font-weight:600">{done}</span> done
      </span>
    </div>
  )
}
