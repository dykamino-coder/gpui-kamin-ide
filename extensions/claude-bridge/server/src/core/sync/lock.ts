const syncQueues = new Map<string, Promise<unknown>>()

function tokenKey(tokenHash: string): string {
  return `token:${tokenHash}`
}

async function runQueued<T>(key: string, operation: () => Promise<T>): Promise<T> {
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

export function withProjectSyncLock<T>(tokenHash: string, _projectPath: string, operation: () => Promise<T>): Promise<T> {
  return runQueued(tokenKey(tokenHash), operation)
}

/** Session creation must not copy a snapshot between its remove and rewrite
 * phases. Await the currently queued user/project uploads before reading. */
export async function waitForSyncSnapshot(tokenHash: string, projectPath?: string): Promise<void> {
  void projectPath
  await syncQueues.get(tokenKey(tokenHash))?.catch(() => undefined)
}
