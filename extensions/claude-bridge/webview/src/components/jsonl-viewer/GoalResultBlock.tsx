import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { renderMarkdown } from '../../utils/render-markdown'

/** Результат проверки цели (`/goal`): Stop-хук возвращает `{ok, reason}`
 *  ОДНОЙ строкой JSON, и в чате это выглядело как сырой дамп на пол-экрана.
 *  Здесь — компактная карточка: статус-строка + свёрнутое обоснование. */
export interface GoalResult {
  ok: boolean
  reason: string
}

/** Распознать запись-результат цели. Строгий тест: ровно объект с булевым
 *  `ok` и строковым `reason` (и не более 3 полей) — случайный JSON от юзера
 *  под это не подпадает. */
export function parseGoalResult(text: string): GoalResult | null {
  const t = text.trim()
  if (!t.startsWith('{') || !t.endsWith('}')) return null
  if (!t.includes('"ok"') || !t.includes('"reason"')) return null
  try {
    const v = JSON.parse(t) as Record<string, unknown>
    if (typeof v.ok !== 'boolean' || typeof v.reason !== 'string') return null
    if (Object.keys(v).length > 3) return null
    return { ok: v.ok, reason: v.reason }
  } catch {
    return null
  }
}

export function GoalResultBlock({ result }: { result: GoalResult }): JSX.Element {
  const [open, setOpen] = useState(false)
  const { ok, reason } = result
  // Первая содержательная строка — заголовок карточки; остальное под катом.
  const lines = reason.split('\n').filter(l => l.trim())
  const head = lines[0] ?? (ok ? 'Цель достигнута' : 'Цель ещё не достигнута')
  const rest = lines.slice(1).join('\n')
  const accent = ok ? 'var(--success, #4ade80)' : 'var(--warning, #fbbf24)'

  return (
    <div
      style={`border:1px solid color-mix(in srgb, ${accent} 35%, transparent);border-radius:10px;background:color-mix(in srgb, ${accent} 7%, transparent);padding:10px 12px;margin:6px 0`}
    >
      <div style="display:flex;align-items:center;gap:8px">
        <i
          class={`fas ${ok ? 'fa-circle-check' : 'fa-circle-half-stroke'}`}
          style={`color:${accent};font-size:13px`}
          aria-hidden="true"
        />
        <span style={`font-weight:600;font-size:12px;color:${accent};letter-spacing:.02em`}>
          {ok ? 'ЦЕЛЬ ДОСТИГНУТА' : 'ЦЕЛЬ В РАБОТЕ'}
        </span>
      </div>
      <div style="margin-top:6px;font-size:13px;color:var(--text-secondary);line-height:1.5">
        {head}
      </div>
      {rest && (
        <>
          <button
            type="button"
            onClick={() => { setOpen(o => !o) }}
            style="margin-top:8px;display:inline-flex;align-items:center;gap:6px;padding:2px 8px;background:transparent;border:none;border-radius:8px;cursor:pointer;color:var(--text-muted-2);font-size:11px"
          >
            <i class={`fas fa-chevron-${open ? 'down' : 'right'}`} aria-hidden="true" />
            {open ? 'Скрыть подробности' : 'Подробности'}
          </button>
          {open && (
            <div
              style="margin-top:6px;font-size:12.5px;color:var(--text-secondary);line-height:1.55"
              dangerouslySetInnerHTML={{ __html: renderMarkdown(rest) }}
            />
          )}
        </>
      )}
    </div>
  )
}
