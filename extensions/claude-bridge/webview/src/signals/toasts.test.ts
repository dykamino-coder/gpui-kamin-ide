import { afterEach, expect, it, vi } from 'vitest'

const transport = vi.hoisted(() => ({
  snd: vi.fn(), inv: vi.fn().mockResolvedValue(''), sub: vi.fn(),
}))
vi.mock('../lib/bridge-transport.js', () => transport)
vi.mock('./connection.js', () => ({ reconnectNonce: { value: 0 } }))
afterEach(() => { vi.unstubAllGlobals(); vi.clearAllMocks() })

it('preserves a requested 6000ms countdown in the actual notification bridge frame', async () => {
  vi.stubGlobal('window', {})
  await import('../bridge-shim')
  const { showToast, toasts } = await import('./toasts')
  showToast({ type: 'info', title: 'Busy', message: 'Try again', duration: 6000 })
  expect(transport.snd).toHaveBeenCalledExactlyOnceWith('show-notification', 'info', 'Busy: Try again', 6000)
  expect(toasts.value).toEqual([])
})
