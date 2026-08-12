const syncQueues = new Map<string, Promise<unknown>>()

function tokenKey(tokenHash: string): string {
  return `token:${tokenHash}`
}

async function runQueued<T>(key: string, operation: () => Promise<T> | T): Promise<T> {
  const previous = syncQueues.get(key)?.catch(() => undefined) ?? Promise.resolve()
  const current = previous.then(operation)
  syncQueues.set(key, current)
  try {
    return await current
  } finally {
    if (syncQueues.get(key) === current) syncQueues.delete(key)
  }
}

export function withUserSyncLock<T>(tokenHash: string, operation: () => Promise<T>): Promise<T> {
  return runQueued(tokenKey(tokenHash), operation)
}

export function withProjectSyncLock<T>(
  tokenHash: string,
  _projectPath: string,
  operation: () => Promise<T>,
): Promise<T> {
  return runQueued(tokenKey(tokenHash), operation)
}

/** Run a session's snapshot copy under the same token-level lock as uploads. */
export function withSyncSnapshotLock<T>(tokenHash: string, operation: () => Promise<T> | T): Promise<T> {
  return runQueued(tokenKey(tokenHash), operation)
}
