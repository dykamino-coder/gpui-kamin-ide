// Счётчики использования тулов за сессию (панель Tools) + fullscreen-фильтр
// применений в чате. Агрегатор НЕ хранит записи — только имя→счётчик с
// дедупом по uuid (реплей и реконнект-повторы не задваивают счёт).
import { signal } from '@preact/signals'
import { stripMcpPrefix } from '../lib/mcp-tool-name'

export interface ToolUsage {
  name: string
  count: number
  lastTs: string
}

const countsByTab = new Map<string, Map<string, ToolUsage>>()
const seenByTab = new Map<string, Set<string>>()

/** Бамп версии — панель перерисовывается по нему, не по глубоким структурам. */
export const toolUsageVersion = signal(0)

/** Тул, чьи применения раскрыты фуллскрином в чате (null = закрыт). */
export const fullscreenToolName = signal<string | null>(null)

export function toolUsageForTab(tabId: string | null): ToolUsage[] {
  if (!tabId) return []
  const m = countsByTab.get(tabId)
  if (!m) return []
  // Свежие сверху: сортировка по времени последнего вызова (запрос юзера),
  // тай-брейк — счётчик и имя.
  return [...m.values()].sort((a, b) =>
    b.lastTs.localeCompare(a.lastTs) || b.count - a.count || a.name.localeCompare(b.name))
}

/** Скормить батч JSONL-записей (chat и tools зовут из одного места приёма). */
export function recordToolUsage(tabId: string, entries: unknown[]): void {
  let changed = false
  let counts = countsByTab.get(tabId)
  let seen = seenByTab.get(tabId)
  for (const raw of entries) {
    const e = raw as { type?: string; uuid?: string; timestamp?: string; message?: { content?: unknown } }
    if (e.type !== 'assistant') continue
    const content = e.message?.content
    if (!Array.isArray(content)) continue
    for (const b of content as Array<{ type?: string; id?: string; name?: unknown }>) {
      if (b?.type !== 'tool_use' || typeof b.name !== 'string') continue
      // Дедуп по id tool_use-блока (уникален), фолбэк на uuid записи.
      const key = b.id || e.uuid
      if (!key) continue
      seen ??= new Set<string>(); if (!seenByTab.has(tabId)) seenByTab.set(tabId, seen)
      if (seen.has(key)) continue
      seen.add(key)
      counts ??= new Map<string, ToolUsage>(); if (!countsByTab.has(tabId)) countsByTab.set(tabId, counts)
      const name = stripMcpPrefix(b.name)
      const cur = counts.get(name)
      // lastTs — монотонный максимум: батчи приходят НЕ хронологично (реплей
      // сабагент-файлов и ренжей идёт после живого хвоста), перезапись «последним
      // пришедшим» откатывала дату к старой и панель казалась замороженной.
      const ts = e.timestamp ?? ''
      if (cur) { cur.count += 1; if (ts > cur.lastTs) cur.lastTs = ts }
      else counts.set(name, { name, count: 1, lastTs: ts })
      changed = true
    }
  }
  if (changed) toolUsageVersion.value += 1
}

/** Полный сброс таба: перед реплеем с нуля и при закрытии вкладки. */
export function resetToolUsage(tabId: string): void {
  const had = countsByTab.delete(tabId)
  seenByTab.delete(tabId)
  if (had) toolUsageVersion.value += 1
}
