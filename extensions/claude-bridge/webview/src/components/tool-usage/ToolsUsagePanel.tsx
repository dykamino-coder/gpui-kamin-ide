// Панель Tools: тулы активной сессии с числом вызовов и временем последнего
// вызова (свежие сверху). Клик раскрывает применения ИНЛАЙН в самой панели
// (аналог панели Agents: Back возвращает к списку) — записи приходят от
// хоста по запросу, панельный стор держит только plan/todo-срез (OOM-диета).
import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { activeTabId } from '../../signals/tabs'
import { toolUsageForTab, toolUsageVersion } from '../../signals/tool-usage'
import { JsonlToolIcon } from '../jsonl-viewer/JsonlToolIcon'
import { JsonlEntry } from '../jsonl-viewer/JsonlEntry'
import type { JsonlEntryData } from '../../types/jsonl'
import { useBridge } from '../../hooks/useBridge'

function lastCallLabel(ts: string): string {
  if (!ts) return ''
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return ''
  const sameDay = new Date().toDateString() === d.toDateString()
  const hm = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
  return sameDay ? hm : `${String(d.getDate()).padStart(2, '0')}.${String(d.getMonth() + 1).padStart(2, '0')} ${hm}`
}

export function ToolsUsagePanel(): JSX.Element {
  const bridge = useBridge()
  void toolUsageVersion.value // подписка на изменения счётчиков
  const tabId = activeTabId.value
  const usage = toolUsageForTab(tabId)
  const [selected, setSelected] = useState<string | null>(null)
  const [entries, setEntries] = useState<JsonlEntryData[]>([])
  const [loading, setLoading] = useState(false)

  // Команда claude-bridge.openToolUsage (e2e) открывает тот же инлайн-вид.
  useEffect(() => bridge.onToolUsageOpen((name) => { setSelected(name) }), [])
  // Смена сессии закрывает просмотр — записи принадлежат прежнему табу.
  useEffect(() => { setSelected(null) }, [tabId])
  useEffect(() => {
    if (!selected || !tabId) { setEntries([]); return }
    let dead = false
    setLoading(true)
    bridge.getToolUsageEntries(tabId, selected)
      .then((r) => { if (!dead) setEntries(r as JsonlEntryData[]) })
      .catch(() => { if (!dead) setEntries([]) })
      .finally(() => { if (!dead) setLoading(false) })
    return () => { dead = true }
  }, [selected, tabId])

  if (selected) {
    return (
      <div style="display:flex;flex-direction:column;flex:1;min-height:0">
        <div style="display:flex;align-items:center;gap:8px;padding:4px 6px 8px">
          <button
            type="button"
            onClick={() => { setSelected(null) }}
            style="display:inline-flex;align-items:center;gap:6px;padding:4px 10px;background:var(--bg-surface);border:none;border-radius:14px;cursor:pointer;color:var(--text-secondary);font-size:12px"
          >
            <i class="fas fa-arrow-left" aria-hidden="true" /> Tools
          </button>
          <span style="display:inline-flex;align-items:center;gap:8px;font-weight:600;font-size:13px">
            <JsonlToolIcon toolName={selected} />
            {selected}
            <span style="color:var(--text-muted-2);font-weight:400"> · {entries.length} entries in window</span>
          </span>
        </div>
        <div style="flex:1;overflow-y:auto;min-height:0;padding:0 2px">
          {loading
            ? <div style="color:var(--text-disabled);font-size:12px;padding:12px">Loading…</div>
            : entries.length === 0
              ? <div style="color:var(--text-disabled);font-size:12px;padding:12px">No calls of this tool in the loaded window.</div>
              : entries.map((entry, i) => <JsonlEntry key={i} entry={entry} />)}
        </div>
      </div>
    )
  }

  if (usage.length === 0) {
    return (
      <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;flex:1;gap:8px;color:var(--text-disabled);text-align:center;padding:24px">
        <i class="fas fa-wrench" style="font-size:26px" />
        <span style="font-size:13px;font-weight:500;color:var(--text-muted)">Tools</span>
        <span style="font-size:12px">Tools used in the active session appear here.</span>
      </div>
    )
  }

  const total = usage.reduce((s, u) => s + u.count, 0)
  return (
    <div style="display:flex;flex-direction:column;gap:2px">
      <div style="font-size:11px;color:var(--text-muted-2);padding:2px 6px 6px">
        {usage.length} tools · {total} calls
      </div>
      {usage.map((u) => (
        <button
          key={u.name}
          type="button"
          title={`Show all ${u.name} calls`}
          onClick={() => { setSelected(u.name) }}
          style="display:flex;align-items:center;gap:8px;padding:6px 8px;background:transparent;border:none;border-radius:8px;cursor:pointer;color:var(--text-secondary);font-size:13px;text-align:left;width:100%"
          onMouseEnter={(e) => { (e.currentTarget as HTMLElement).style.background = 'var(--bg-surface)' }}
          onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.background = 'transparent' }}
        >
          <JsonlToolIcon toolName={u.name} />
          <span style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{u.name}</span>
          <span style="font-size:10px;color:var(--text-muted-2);white-space:nowrap">{lastCallLabel(u.lastTs)}</span>
          <span style="font-size:11px;color:var(--text-muted-2);background:var(--bg-surface);border-radius:9px;padding:1px 7px;min-width:22px;text-align:center">{u.count}</span>
        </button>
      ))}
    </div>
  )
}
