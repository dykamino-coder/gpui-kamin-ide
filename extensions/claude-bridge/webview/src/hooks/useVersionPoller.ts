import { useEffect } from 'preact/hooks'
import { useBridge } from './useBridge'
import { updateInfo } from '../signals/ui'

const POLL_INTERVAL_MS = 5 * 60 * 1000  // 5 minutes

/** Compare semantic version strings "a.b.c". Returns >0 if a > b. */
function cmpVersion(a: string, b: string): number {
  const pa = a.split('.').map(n => parseInt(n, 10) || 0)
  const pb = b.split('.').map(n => parseInt(n, 10) || 0)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (d !== 0) return d
  }
  return 0
}

/** Poll `/api/bridge-version` periodically and update `updateInfo` signal
 *  whenever the server has a strictly newer build than the running app. */
export function useVersionPoller(): void {
  const bridge = useBridge()

  useEffect(() => {
    let localVersion = ''
    try { localVersion = bridge.getVersion() } catch { /* not available yet */ }

    async function probe(): Promise<void> {
      try {
        const cfg = await bridge.getConfig()
        if (!cfg?.serverUrl) return
        const httpBase = cfg.serverUrl
          .replace(/^ws(s?):\/\//, 'http$1://')
          .replace(/\/ws\/session$/, '')
          .replace(/\/+$/, '')
        // Via the ext-host (Node) — webview CSP blocks a direct http fetch.
        const data = await (bridge as unknown as {
          serverVersion: (httpBase: string) => Promise<{ version: string } | null>
        }).serverVersion(httpBase)
        if (!data?.version || !localVersion) return
        if (cmpVersion(data.version, localVersion) > 0) {
          const prev = updateInfo.value
          if (!prev || prev.serverVersion !== data.version) {
            updateInfo.value = {
              localVersion,
              serverVersion: data.version,
              state: 'idle',
            }
          }
        } else if (updateInfo.value) {
          // Server caught back up (downgrade on container side) — drop the badge.
          updateInfo.value = null
        }
      } catch { /* ignore transient failures */ }
    }

    probe()
    const tick = setInterval(probe, POLL_INTERVAL_MS)
    const onFocus = () => probe()
    window.addEventListener('focus', onFocus)

    const unsubState = bridge.onUpdateState?.((s) => {
      const cur = updateInfo.value
      if (!cur) return
      updateInfo.value = { ...cur, state: s.state, error: s.error }
    })

    return () => {
      clearInterval(tick)
      window.removeEventListener('focus', onFocus)
      unsubState?.()
    }
  }, [])
}
