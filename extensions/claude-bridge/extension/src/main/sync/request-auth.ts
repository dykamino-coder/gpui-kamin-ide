export interface ServerFetchInit {
  method?: string
  body?: string
  headers?: Record<string, string>
}

/** Sync routes always authenticate with the token owned by the client host. */
export function withSyncAuthorization(
  path: string,
  init: ServerFetchInit | undefined,
  token: string | undefined,
): ServerFetchInit | undefined {
  if (!path.startsWith('/api/sync/') || !token) return init
  return {
    ...init,
    headers: { ...(init?.headers ?? {}), Authorization: `Bearer ${token}` },
  }
}
