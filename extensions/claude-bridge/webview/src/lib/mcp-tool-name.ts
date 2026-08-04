// An MCP-exposed tool arrives with a server-qualified name
// (`mcp__<server>__TaskCreate`); the plan/todos/files derivations and the
// non-chat store projection all match on the BARE tool name. Kept in ONE place
// so the projection filter (jsonl-project.ts) can never drift out of sync with
// what plan.ts / todos.ts actually read — a mismatch would silently empty the
// Plan/Todos panels with no error.
export function stripMcpPrefix(raw: string): string {
  const m = raw.match(/^mcp__[^_]+(?:_[^_]+)*__(.+)$/)
  return m ? m[1]! : raw
}
