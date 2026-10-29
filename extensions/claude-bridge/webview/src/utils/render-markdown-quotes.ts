// Контейнеры цитат CommonMark §5.1; fenced code имеет приоритет над маркером >.
// Ленивые продолжения без маркера намеренно остаются за пределами контейнера.
export function extractBlockquotes(text: string, render: (body: string) => string): string {
  const lines = text.split('\n')
  const out: string[] = []
  let fence: string | undefined
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const delimiter = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line)
    if (fence) {
      out.push(line)
      if (delimiter && delimiter[1][0] === fence[0] && delimiter[1].length >= fence.length && !delimiter[2].trim()) {
        fence = undefined
      }
      continue
    }
    if (delimiter && (delimiter[1][0] !== '`' || !delimiter[2].includes('`'))) {
      fence = delimiter[1]
      out.push(line)
      continue
    }
    const quote = /^ {0,3}>[ \t]?(.*)$/.exec(line)
    if (!quote) {
      out.push(line)
      continue
    }
    const body = [quote[1]]
    while (i + 1 < lines.length) {
      const next = /^ {0,3}>[ \t]?(.*)$/.exec(lines[i + 1])
      if (!next) break
      body.push(next[1])
      i++
    }
    // Цитата может прерывать абзац без пустой строки с обеих сторон.
    out.push(`\n${render(body.join('\n'))}\n`)
  }
  return out.join('\n')
}
