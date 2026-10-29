import type { JsonlEntryData } from '../../types/jsonl'

export interface TaskNotification {
  summary: string
  status: 'completed' | 'failed' | 'running'
  taskId: string
  result?: string
  fields: Array<[string, string]>
}

function decode(text: string): string {
  return text.replace(/&(#x[0-9a-f]+|#\d+|amp|lt|gt|quot|apos);/gi, (entity, name: string) => {
    const named: Record<string, string> = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" }
    if (name[0] !== '#') return named[name.toLowerCase()] ?? entity
    const hex = name[1].toLowerCase() === 'x'
    const point = Number.parseInt(name.slice(hex ? 2 : 1), hex ? 16 : 10)
    return point > 0 && point <= 0x10ffff && !(point >= 0xd800 && point <= 0xdfff)
      ? String.fromCodePoint(point) : entity
  })
}

/** CLI использует XML-обёртку, но result содержит буквальный код с < и >.
 *  Разбираем только поля верхнего уровня, никогда не интерпретируем вывод как HTML. */
export function parseTaskNotification(text: string): TaskNotification | null {
  const root = /^\s*<task-notification>([\s\S]*)<\/task-notification>\s*$/.exec(text)
  if (!root) return null
  const values = new Map<string, string>()
  let rest = root[1].trim()
  const allowed = new Set(['task-id', 'status', 'summary', 'result', 'output-file', 'tool-use-id', 'usage'])
  while (rest) {
    const field = /^<([a-z][\w-]*)>([\s\S]*?)<\/\1>/.exec(rest)
    if (!field || !allowed.has(field[1]) || values.has(field[1])) return null
    const raw = field[2].replace(/^<!\[CDATA\[([\s\S]*)\]\]>$/, '$1')
    values.set(field[1], field[2].startsWith('<![CDATA[') ? raw : decode(raw))
    rest = rest.slice(field[0].length).trim()
  }
  const status = values.get('status')?.trim()
  if (status !== 'completed' && status !== 'failed' && status !== 'running') return null
  const taskId = values.get('task-id')?.trim()
  if (!taskId) return null
  return {
    summary: values.get('summary')?.trim() || `Task ${status}`,
    status, taskId, result: values.get('result'),
    fields: [...values].filter(([key]) => !['summary', 'status', 'task-id', 'result'].includes(key)),
  }
}

/** Отдельная служебная запись, включая неизвестный/повреждённый payload.
 *  Смешанный текст пользователя сохраняет обычную карточку и границу хода. */
export function taskNotificationText(entry: JsonlEntryData): string | null {
  if (entry.type !== 'user' && entry.type !== 'system') return null
  const content = entry.type === 'system' ? entry.content : entry.message?.content
  if (Array.isArray(content) && content.some(b => b.type !== 'text')) return null
  const text = typeof content === 'string' ? content
    : Array.isArray(content) ? content.map(b => b.text || '').join('\n') : ''
  const clean = text.replace(/\x1b\[[0-9;]*m/g, '').trim()
  return /^<task-notification>/.test(clean)
    && (!clean.includes('</task-notification>') || clean.endsWith('</task-notification>')) ? clean : null
}
