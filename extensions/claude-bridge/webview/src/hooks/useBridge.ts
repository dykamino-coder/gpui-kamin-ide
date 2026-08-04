import type { ElectronBridge } from '../../shared/types'

declare global {
  interface Window {
    electronBridge: ElectronBridge
  }
}

export function useBridge(): ElectronBridge {
  return window.electronBridge
}
