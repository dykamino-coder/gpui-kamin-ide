import { beforeEach, describe, expect, it, vi } from 'vitest'

beforeEach(() => vi.resetModules())
describe('BR-04 switches across child generations in a retained frame', () => {
  it('accepts a reset sequence from the new child and rejects delayed dead-child switches', async () => {
    const { activeTabId, applyTabSwitch, switchTabLocal } = await import('./tabs')
    expect(applyTabSwitch('old', 91, 4)).toBe(true)
    switchTabLocal('stale-intent')
    expect(applyTabSwitch('restored', 1, 5)).toBe(true)
    expect(activeTabId.value).toBe('restored')
    expect(applyTabSwitch('dead', 92, 4)).toBe(false)
    expect(applyTabSwitch('dead-legacy', 100)).toBe(false)
    expect(activeTabId.value).toBe('restored')
    expect(applyTabSwitch('next', 2, 5)).toBe(true)
    expect(applyTabSwitch('late', 1, 5)).toBe(false)
  })
})
