import type { KaminBridgeApi } from '../../shared/types'

declare global {
  interface Window {
    /** @deprecated Compatibility alias for older vendored renderer code. */
    electronBridge: KaminBridgeApi
    kaminBridge: KaminBridgeApi
  }
}

export function useBridge(): KaminBridgeApi {
  return window.kaminBridge ?? window.electronBridge
}
