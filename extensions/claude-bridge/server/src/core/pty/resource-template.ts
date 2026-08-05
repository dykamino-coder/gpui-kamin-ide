/**
 * Conservatively match a concrete resource URI against an RFC 6570-style
 * template. The bridge only needs routing, not variable extraction: the
 * concrete upstream URI is forwarded unchanged to the owning MCP server.
 */
export function matchesResourceTemplate(template: string, uri: string): boolean {
  const literals: string[] = []
  let offset = 0
  let expressionCount = 0
  while (offset < template.length) {
    const open = template.indexOf('{', offset)
    if (open < 0) break
    const close = template.indexOf('}', open + 1)
    if (close < 0 || close === open + 1 || template.slice(open + 1, close).includes('{')) return false
    literals.push(template.slice(offset, open))
    expressionCount++
    offset = close + 1
  }
  if (template.slice(offset).includes('}')) return false
  if (expressionCount === 0) return template === uri
  literals.push(template.slice(offset))

  const head = literals[0] ?? ''
  if (!uri.startsWith(head)) return false
  let uriOffset = head.length
  for (let index = 1; index < literals.length - 1; index++) {
    const literal = literals[index]
    if (!literal) continue
    const found = uri.indexOf(literal, uriOffset)
    if (found < 0) return false
    uriOffset = found + literal.length
  }
  const tail = literals[literals.length - 1]
  return tail ? uri.endsWith(tail) && uri.length - tail.length >= uriOffset : true
}
