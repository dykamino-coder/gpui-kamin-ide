import type { TabInfo } from '../../shared/types'

export const SESSION_LIMIT_NOTICE = 'Close another connected chat using this Bridge token. This chat will retry automatically.'

export function waitingForSessionSlot(tab?: Pick<TabInfo, 'status' | 'refusal'>): boolean {
  return tab?.refusal === 'session-limit' && tab.status !== 'connected'
}
