import type { JSX } from 'preact'

const LONG_STRING = 120

/** Красивый generic-рендер входа тула вместо сырого `JSON.stringify`:
 *  таблица ключ-значение — строки как текст (длинные — pre-wrap блоком),
 *  примитивы значением, вложенные объекты — свёрнутым JSON. Закрывает все
 *  тулы без спец-виджета (Read, Grep, LspDiagnostics, browser-тулы, ToolSearch…) —
 *  аудит транскрипта: «неизвестных» форм в чате остаться не должно. */
export function GenericToolInput({ input }: { input: any }): JSX.Element {
  if (input === null || input === undefined) return <></>
  if (typeof input !== 'object' || Array.isArray(input)) {
    return <>{JSON.stringify(input, null, 2)}</>
  }
  const entries = Object.entries(input as Record<string, unknown>)
  if (entries.length === 0) return <></>
  return (
    <div style="white-space:normal;display:flex;flex-direction:column;gap:3px">
      {entries.map(([k, v]) => {
        let val: JSX.Element
        if (typeof v === 'string') {
          val = v.length > LONG_STRING || v.includes('\n')
            ? <div style="white-space:pre-wrap;font-size:11px;color:var(--text-secondary);max-height:240px;overflow-y:auto;border-left:2px solid var(--border-subtle);padding-left:8px;margin-top:2px">{v}</div>
            : <span style="color:var(--text-secondary)">{v}</span>
        } else if (typeof v === 'number' || typeof v === 'boolean') {
          val = <span style="color:var(--accent-blue, var(--text-secondary))">{String(v)}</span>
        } else {
          val = <span style="color:var(--text-muted);font-size:11px;font-family:var(--font-mono)">{JSON.stringify(v)}</span>
        }
        return (
          <div key={k} style="display:flex;gap:8px;align-items:baseline;flex-wrap:wrap">
            <span style="color:var(--text-muted);font-size:11px;font-weight:600;flex-shrink:0">{k}</span>
            <div style="min-width:0;flex:1">{val}</div>
          </div>
        )
      })}
    </div>
  )
}
