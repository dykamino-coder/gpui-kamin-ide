import type { JSX } from 'preact'
import { useEffect, useMemo, useRef, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { copyWithToast } from '../../../lib/clipboard'
import styles from './LogsPanel.module.css'

interface LogEntry {
  ts: number
  level: 'error' | 'warn' | 'info'
  source: string
  message: string
  stack?: string
}

type LevelFilter = 'all' | 'error' | 'warn' | 'info'

function fmtTime(ts: number): string {
  const d = new Date(ts)
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  const ss = String(d.getSeconds()).padStart(2, '0')
  return `${hh}:${mm}:${ss}`
}

export function LogsPanel(): JSX.Element {
  const bridge = useBridge()
  const [entries, setEntries] = useState<LogEntry[]>([])
  const [filter, setFilter] = useState<LevelFilter>('all')
  const [search, setSearch] = useState('')
  const [expanded, setExpanded] = useState<number | null>(null)
  const [stickBottom, setStickBottom] = useState(true)
  const listRef = useRef<HTMLDivElement>(null)

  // Load initial snapshot.
  useEffect(() => {
    let cancelled = false
    bridge?.listLogs?.().then((list: LogEntry[]) => {
      if (!cancelled) setEntries(list || [])
    }).catch(() => {})
    return () => { cancelled = true }
  }, [])

  // Live append new entries as they happen.
  useEffect(() => {
    if (!bridge?.onLogEntry) return
    return bridge.onLogEntry((entry: LogEntry) => {
      setEntries(prev => {
        const next = [...prev, entry]
        if (next.length > 500) next.shift()
        return next
      })
    })
  }, [])

  // Auto-scroll stick logic.
  useEffect(() => {
    if (!stickBottom) return
    const el = listRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [entries, stickBottom])

  function onScroll(e: Event): void {
    const el = e.currentTarget as HTMLDivElement
    const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 20
    setStickBottom(atBottom)
  }

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase()
    return entries.filter(e => {
      if (filter !== 'all' && e.level !== filter) return false
      if (q && !(e.message.toLowerCase().includes(q) || e.source.toLowerCase().includes(q))) return false
      return true
    })
  }, [entries, filter, search])

  async function handleClear(): Promise<void> {
    await bridge?.clearLogs?.()
    setEntries([])
    setExpanded(null)
  }

  async function handleCopy(): Promise<void> {
    const text = filtered.map(e => {
      const head = `[${fmtTime(e.ts)}] [${e.level.toUpperCase()}] [${e.source}] ${e.message}`
      return e.stack ? `${head}\n${e.stack}` : head
    }).join('\n')
    await copyWithToast(text, 'Logs copied')
  }

  const counts = useMemo(() => ({
    error: entries.filter(e => e.level === 'error').length,
    warn: entries.filter(e => e.level === 'warn').length,
    info: entries.filter(e => e.level === 'info').length,
  }), [entries])

  return (
    <div class={styles.panel}>
      <div class={styles.toolbar}>
        <FilterChip label={`All (${entries.length})`} active={filter === 'all'} onClick={() => setFilter('all')} />
        <FilterChip label={`Errors (${counts.error})`} active={filter === 'error'} color="var(--accent-red)" onClick={() => setFilter('error')} />
        <FilterChip label={`Warnings (${counts.warn})`} active={filter === 'warn'} color="var(--accent-yellow)" onClick={() => setFilter('warn')} />
        <FilterChip label={`Info (${counts.info})`} active={filter === 'info'} color="var(--accent-primary)" onClick={() => setFilter('info')} />
        <input
          class={styles.search}
          type="text"
          placeholder="Filter by text or source…"
          value={search}
          onInput={(e) => setSearch((e.target as HTMLInputElement).value)}
        />
        <button class={styles.btn} onClick={handleCopy} title="Copy filtered log to clipboard">
          <i class="fas fa-copy" /> Copy
        </button>
        <button class={styles.btn} onClick={handleClear} title="Clear log buffer">
          <i class="fas fa-trash" /> Clear
        </button>
      </div>
      <div class={styles.list} ref={listRef} onScroll={onScroll}>
        {filtered.length === 0 ? (
          <div class={styles.empty}>
            {entries.length === 0
              ? 'No events captured yet. Errors and warnings surface here as they happen.'
              : 'No entries match the current filter.'}
          </div>
        ) : filtered.map((e, i) => (
          <LogRow key={`${e.ts}-${i}`} entry={e} expanded={expanded === i} onToggle={() => setExpanded(expanded === i ? null : i)} />
        ))}
      </div>
    </div>
  )
}

function FilterChip({ label, active, color, onClick }: { label: string; active: boolean; color?: string; onClick: () => void }): JSX.Element {
  return (
    <button
      class={`${styles.chip} ${active ? styles.chipActive : ''}`}
      style={active && color ? `border-color:${color};color:${color}` : undefined}
      onClick={onClick}
    >
      {label}
    </button>
  )
}

function LogRow({ entry, expanded, onToggle }: { entry: LogEntry; expanded: boolean; onToggle: () => void }): JSX.Element {
  const levelColor = entry.level === 'error' ? 'var(--accent-red)' : entry.level === 'warn' ? 'var(--accent-yellow)' : 'var(--accent-primary)'
  const hasDetail = Boolean(entry.stack)
  return (
    <div class={`${styles.row} ${expanded ? styles.rowExpanded : ''}`} onClick={hasDetail ? onToggle : undefined}>
      <span class={styles.time}>{fmtTime(entry.ts)}</span>
      <span class={styles.level} style={`color:${levelColor}`}>{entry.level.toUpperCase()}</span>
      <span class={styles.source}>{entry.source}</span>
      <span class={styles.msg}>{entry.message}</span>
      {hasDetail && <i class={`fas ${expanded ? 'fa-chevron-down' : 'fa-chevron-right'} ${styles.arrow}`} />}
      {expanded && entry.stack && (
        <pre class={styles.stack}>{entry.stack}</pre>
      )}
    </div>
  )
}
