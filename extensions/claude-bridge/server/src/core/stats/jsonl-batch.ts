// Пачка строк `jsonl_events` перед вставкой.
//
// `INSERT … ON CONFLICT DO NOTHING` защищает только от строк, которые УЖЕ
// закоммичены. Два одинаковых uuid внутри одной транзакции DuckDB замечает
// лишь на COMMIT, и неудачный COMMIT валит процесс (см. `database/write-lock.ts`).
// JSONL Claude CLI такие повторы содержит: после `/resume` и компакции запись
// с тем же uuid встречается в файле дважды. Поэтому повторы снимаются здесь,
// до того как строка дойдёт до базы.

/** Оставить первое вхождение каждого uuid (колонка 0); порядок сохраняется. */
export function dedupeByUuid<T extends unknown[]>(rows: readonly T[]): T[] {
  const seen = new Set<unknown>()
  const out: T[] = []
  for (const row of rows) {
    const uuid = row[0]
    if (seen.has(uuid)) continue
    seen.add(uuid)
    out.push(row)
  }
  return out
}
