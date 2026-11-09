import type { KaminBridgeApi } from '../../shared/types'

declare global {
  interface Window {
    kaminBridge: KaminBridgeApi
  }
}

export function useBridge(): KaminBridgeApi {
  return window.kaminBridge
}
