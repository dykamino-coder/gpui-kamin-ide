import type { JSX } from 'preact'
import { useEffect, useRef, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { ListSkeleton } from '../../ui/ListSkeleton'
import { PanelError } from '../../ui/PanelError'
import styles from './MonitorsPanel.module.css'

interface Monitor {
  id: string
  tabId: string
  pluginId: string
  pluginName: string
  monitorName: string
  description: string
  command: string
  status: 'running' | 'exited' | 'error'
  startedAt: number
  exitCode?: number | null
}

interface LogEntry { ts: number; stream: string; text: string }

function formatTime(ts: number): string {
  const d = new Date(ts)
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  const ss = String(d.getSeconds()).padStart(2, '0')
  return `${hh}:${mm}:${ss}`
}

export function MonitorsPanel(): JSX.Element {
  const bridge = useBridge()
  const [monitors, setMonitors] = useState<Monitor[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [log, setLog] = useState<LogEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)
  const listRef = useRef<HTMLDivElement>(null)
  const stickToBottom = useRef(true)

  async function refreshList(): Promise<void> {
    setLoadError(false)
    try {
      const list = (await bridge.listMonitors()) ?? [] // unimplemented channel can resolve null
      setMonitors(list)
      if (list.length > 0 && !selectedId) {
        setSelectedId(list[0]!.id)
      }
    } catch {
      setLoadError(true)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { void refreshList() }, [])

  // Live list updates: mount/unmount and status changes rebuild the chip bar.
  useEffect(() => {
    const offStart = bridge.onMonitorStarted(() => refreshList())
    const offStop = bridge.onMonitorStopped(({ id }) => {
      if (id === selectedId) setSelectedId(null)
      refreshList()
    })
    const offStatus = bridge.onMonitorStatus(() => refreshList())
    return () => { offStart(); offStop(); offStatus() }
  }, [selectedId])

  // Load ring buffer for the selected monitor + subscribe to output events.
  useEffect(() => {
    if (!selectedId) { setLog([]); return }
    let cancelled = false
    bridge.getMonitorLog(selectedId).then((entries) => {
      if (!cancelled) setLog(entries ?? [])
    })
    const off = bridge.onMonitorOutput(({ id, entry }) => {
      if (id !== selectedId) return
      setLog(prev => [...prev, entry as LogEntry])
    })
    return () => { cancelled = true; off() }
  }, [selectedId])

  useEffect(() => {
    const el = listRef.current
    if (!el || !stickToBottom.current) return
    el.scrollTop = el.scrollHeight
  }, [log])

  function handleScroll(): void {
    const el = listRef.current
    if (!el) return
    stickToBottom.current = (el.scrollHeight - el.scrollTop - el.clientHeight) < 24
  }

  const current = monitors.find(m => m.id === selectedId) || null

  return (
    <div class={styles.panel}>
      <h2 class={styles.title}>Plugin monitors</h2>
      <p class={styles.subtitle}>Long-running background processes declared by enabled plugins. Each spawns when a tab opens and dies when it closes.</p>

      {loading && monitors.length === 0 ? (
        <ListSkeleton rows={4} />
      ) : loadError && monitors.length === 0 ? (
        <PanelError message="Could not load monitors." onRetry={() => { setLoading(true); void refreshList() }} />
      ) : monitors.length === 0 ? (
        <div class={styles.empty}>No active monitors. Open a tab so plugins with <code>monitors</code> in their manifest can start.</div>
      ) : (
        <>
          <div class={styles.chips}>
            {monitors.map(m => (
              <button
                key={m.id}
                class={`${styles.chip} ${m.id === selectedId ? styles.chipActive : ''} ${styles[`chip-${m.status}`] ?? ''}`}
                onClick={() => setSelectedId(m.id)}
                title={`${m.pluginName}: ${m.description || m.monitorName}`}
              >
                <span class={styles.dot} />
                <span class={styles.chipLabel}>{m.monitorName}</span>
                <span class={styles.chipPlugin}>@{m.pluginName}</span>
              </button>
            ))}
          </div>

          {current && (
            <div class={styles.detailHeader}>
              <div>
                <div class={styles.monitorName}>{current.monitorName}</div>
                <div class={styles.monitorMeta}>
                  plugin: {current.pluginId} · status: {current.status}
                  {current.exitCode != null && ` · exit code: ${current.exitCode}`}
                </div>
                {current.description && <div class={styles.monitorDesc}>{current.description}</div>}
                <div class={styles.monitorCmd}><code>{current.command}</code></div>
              </div>
            </div>
          )}

          <div class={styles.log} ref={listRef} onScroll={handleScroll}>
            {log.length === 0 && <div class={styles.logEmpty}>No output yet.</div>}
            {log.map((e, i) => (
              <div key={i} class={`${styles.row} ${styles[`stream-${e.stream}`] ?? ''}`}>
                <span class={styles.ts}>{formatTime(e.ts)}</span>
                <span class={styles.stream}>{e.stream}</span>
                <span class={styles.text}>{e.text}</span>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  )
}
