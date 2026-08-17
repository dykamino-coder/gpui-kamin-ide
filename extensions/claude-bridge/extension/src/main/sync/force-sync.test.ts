import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  resetSyncTimers: vi.fn(),
  syncUserData: vi.fn(),
  syncProjectData: vi.fn(),
}))

vi.mock('./sync-client', () => mocks)

import { forceSync } from './force-sync'

describe('forceSync', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocks.syncUserData.mockResolvedValue({ ok: true })
    mocks.syncProjectData.mockResolvedValue({ ok: true })
  })

  it('rejects an unconfigured connection', async () => {
    const result = await forceSync({
      configStore: { get: () => ({ serverUrl: '', token: '' }) },
      getProjectPaths: () => [],
    })

    expect(result).toEqual({ ok: false, error: 'Server URL or token not configured' })
    expect(mocks.resetSyncTimers).not.toHaveBeenCalled()
  })

  it('uploads user data and every distinct project immediately', async () => {
    const result = await forceSync({
      configStore: { get: () => ({ serverUrl: 'ws://bridge:3456', token: 'owner-secret' }) },
      getProjectPaths: () => ['C:\\repo', 'C:\\repo', 'D:\\other'],
    })

    expect(mocks.resetSyncTimers).toHaveBeenCalledOnce()
    expect(mocks.syncUserData).toHaveBeenCalledWith('ws://bridge:3456', 'owner-secret')
    expect(mocks.syncProjectData.mock.calls).toEqual([
      ['ws://bridge:3456', 'owner-secret', 'C:\\repo'],
      ['ws://bridge:3456', 'owner-secret', 'D:\\other'],
    ])
    expect(result).toEqual({ ok: true, projectPath: 'C:\\repo' })
  })

  it('returns the upload error instead of reporting a false success', async () => {
    mocks.syncUserData.mockResolvedValue({ ok: false, error: 'User sync failed: 401 Unauthorized' })

    const result = await forceSync({
      configStore: { get: () => ({ serverUrl: 'ws://bridge:3456', token: 'owner-secret' }) },
      getProjectPaths: () => ['C:\\repo'],
    })

    expect(result).toEqual({ ok: false, error: 'User sync failed: 401 Unauthorized' })
    expect(mocks.syncProjectData).not.toHaveBeenCalled()
  })
})
