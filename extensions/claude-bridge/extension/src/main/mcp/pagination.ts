export const DEFAULT_MCP_LIST_MAX_PAGES = 100
export const DEFAULT_MCP_LIST_MAX_ITEMS = 10_000

type ListRequest = (method: string, params: Record<string, unknown>) => Promise<unknown>

export interface McpListOptions {
  maxPages?: number
  maxItems?: number
}

/**
 * Fetch every page of an MCP list method without trusting a remote server to
 * eventually terminate its cursor chain. The returned catalog is all-or-
 * nothing: malformed responses, cursor cycles and configured limits reject so
 * callers do not replace a known-good catalog with a silent partial result.
 */
export async function collectMcpList<T>(
  request: ListRequest,
  method: string,
  itemKey: string,
  options: McpListOptions = {},
): Promise<T[]> {
  const maxPages = options.maxPages ?? DEFAULT_MCP_LIST_MAX_PAGES
  const maxItems = options.maxItems ?? DEFAULT_MCP_LIST_MAX_ITEMS
  if (!Number.isSafeInteger(maxPages) || maxPages < 1) throw new Error('MCP list maxPages must be a positive integer')
  if (!Number.isSafeInteger(maxItems) || maxItems < 1) throw new Error('MCP list maxItems must be a positive integer')

  const items: T[] = []
  const seenCursors = new Set<string>()
  let cursor: string | undefined

  for (let page = 1; page <= maxPages; page++) {
    const params = cursor === undefined ? {} : { cursor }
    const raw = await request(method, params)
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
      throw new Error(`${method} returned a malformed page`)
    }
    const result = raw as Record<string, unknown>
    const pageItems = result[itemKey]
    if (pageItems !== undefined && !Array.isArray(pageItems)) {
      throw new Error(`${method} returned non-array ${itemKey}`)
    }
    if (Array.isArray(pageItems)) {
      if (items.length + pageItems.length > maxItems) {
        throw new Error(`${method} exceeded the ${maxItems}-item catalog limit`)
      }
      items.push(...pageItems as T[])
    }

    const nextCursor = result.nextCursor
    if (nextCursor === undefined || nextCursor === null || nextCursor === '') return items
    if (typeof nextCursor !== 'string') throw new Error(`${method} returned a non-string nextCursor`)
    if (seenCursors.has(nextCursor)) throw new Error(`${method} returned a repeated cursor`)
    seenCursors.add(nextCursor)
    cursor = nextCursor
  }

  throw new Error(`${method} exceeded the ${maxPages}-page pagination limit`)
}
