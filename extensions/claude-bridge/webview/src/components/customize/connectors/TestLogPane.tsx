import type { JSX } from 'preact'
import { useEffect, useRef, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { copyWithToast } from '../../../lib/clipboard'
import styles from './TestLogPane.module.css'

interface LogEntry { ts: number; level: string; msg: string }

interface Props { serverId: string }

function formatTime(ts: number): string {
  const d = new Date(ts)
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  const ss = String(d.getSeconds()).padStart(2, '0')
  const ms = String(d.getMilliseconds()).padStart(3, '0')
  return `${hh}:${mm}:${ss}.${ms}`
}

export function TestLogPane({ serverId }: Props): JSX.Element {
  const bridge = useBridge()
  const [entries, setEntries] = useState<LogEntry[]>([])
  const listRef = useRef<HTMLDivElement>(null)
  const stickToBottom = useRef(true)

  // Load existing buffer on mount + whenever the selected server changes.
  // The buffer survives across Test clicks, so re-opening the pane shows
  // the last connect trace even if the user didn't press Test again.
  useEffect(() => {
    let cancelled = false
    bridge.getMcpTestLog(serverId).then((log) => {
      // Unimplemented host channel resolves null — render an empty log, not a crash.
      if (!cancelled) setEntries(log ?? [])
    }).catch(() => {})
    return () => { cancelled = true }
  }, [serverId])

  useEffect(() => {
    const offLog = bridge.onMcpTestLog(({ id, entry }) => {
      if (id !== serverId) return
      setEntries((prev) => [...prev, entry])
    })
    const offClear = bridge.onMcpTestLogCleared(({ id }) => {
      if (id !== serverId) return
      setEntries([])
    })
    return () => { offLog(); offClear() }
  }, [serverId])

  // Auto-scroll to the bottom when new lines arrive, but only if the user
  // hadn't scrolled up — respecting a manual position is standard terminal
  // UX (see xterm.js). 24px threshold tolerates sub-pixel rounding.
  useEffect(() => {
    const el = listRef.current
    if (!el || !stickToBottom.current) return
    el.scrollTop = el.scrollHeight
  }, [entries])

  function handleScroll(): void {
    const el = listRef.current
    if (!el) return
    stickToBottom.current = (el.scrollHeight - el.scrollTop - el.clientHeight) < 24
  }

  async function handleClear(): Promise<void> {
    await bridge.clearMcpTestLog(serverId)
  }

  async function handleCopy(): Promise<void> {
    const text = entries.map(e => `${formatTime(e.ts)} [${e.level}] ${e.msg}`).join('\n')
    await copyWithToast(text, 'Log copied')
  }

  return (
    <div class={styles.pane}>
      <div class={styles.header}>
        <span class={styles.title}>Connection log</span>
        <span class={styles.count}>{entries.length} line{entries.length === 1 ? '' : 's'}</span>
        <div class={styles.spacer} />
        <button class={styles.btn} onClick={handleCopy} disabled={entries.length === 0}>Copy</button>
        <button class={styles.btn} onClick={handleClear} disabled={entries.length === 0}>Clear</button>
      </div>
      <div class={styles.list} ref={listRef} onScroll={handleScroll}>
        {entries.length === 0 && (
          <div class={styles.empty}>No log yet. Hit Test to see what happens during connect.</div>
        )}
        {entries.map((e, i) => (
          <div key={i} class={`${styles.row} ${styles[`lvl-${e.level}`] ?? ''}`}>
            <span class={styles.ts}>{formatTime(e.ts)}</span>
            <span class={styles.level}>{e.level}</span>
            <span class={styles.msg}>{e.msg}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
