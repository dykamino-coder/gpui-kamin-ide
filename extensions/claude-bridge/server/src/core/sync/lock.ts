const userSyncQueues = new Map<string, Promise<unknown>>()

async function runQueued<T>(key: string, operation: () => Promise<T>): Promise<T> {
  const previous = userSyncQueues.get(key)?.catch(() => undefined) ?? Promise.resolve()
  const current = previous.then(operation)
  userSyncQueues.set(key, current)
  try {
    return await current
  } finally {
    if (userSyncQueues.get(key) === current) userSyncQueues.delete(key)
  }
}

/** Serialize user uploads because each one replaces the complete plugin tree. */
export function withUserSyncLock<T>(tokenHash: string, operation: () => Promise<T>): Promise<T> {
  return runQueued(tokenHash, operation)
}

/** Avoid copying a plugin snapshot between its remove and rewrite phases. */
export async function waitForUserSyncSnapshot(tokenHash: string): Promise<void> {
  await userSyncQueues.get(tokenHash)?.catch(() => undefined)
}
