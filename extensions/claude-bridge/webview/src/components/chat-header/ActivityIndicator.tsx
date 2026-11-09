import type { JSX } from 'preact'
import { useEffect, useRef, useState } from 'preact/hooks'
import styles from './ActivityIndicator.module.css'

interface ActivityIndicatorProps {
  visible: boolean
  text: string
  thinking?: string
  /** Spinner highlight colour — defaults to the global `--accent-primary`,
   *  pass the active tab's per-agent colour to make the indicator match
   *  the chat it belongs to. */
  color?: string
}

function truncateThinking(s: string): string {
  const oneLine = s.replace(/\s+/g, ' ').trim()
  return oneLine.length > 120 ? oneLine.slice(0, 120) + '…' : oneLine
}

/**
 * Replace the "(Ns · …)" seconds prefix with a client-side live counter.
 * The CLI prints the status line only every few seconds, so without this the
 * number looks frozen. We anchor on the last server-reported value and tick
 * locally once per second.
 */
function useLiveText(rawText: string, visible: boolean): string {
  const anchor = useRef<{ serverSec: number; receivedAt: number; rawText: string } | null>(null)
  const [, setTick] = useState(0)

  useEffect(() => {
    if (!visible) { anchor.current = null; return }
    // Extract leading "(Ns" — the first number before 's'
    const m = rawText.match(/\((\d+)s/)
    if (m) {
      anchor.current = { serverSec: parseInt(m[1]!, 10), receivedAt: Date.now(), rawText }
    } else {
      anchor.current = null
    }
  }, [rawText, visible])

  useEffect(() => {
    if (!visible) return
    const id = setInterval(() => setTick(t => t + 1), 500)
    return () => clearInterval(id)
  }, [visible])

  if (!anchor.current) return rawText
  const a = anchor.current
  const elapsed = a.serverSec + Math.floor((Date.now() - a.receivedAt) / 1000)
  return a.rawText.replace(/\(\d+s/, `(${elapsed}s`)
}

/** Glyph spinner frames, dot → star → dot (the Claude Code spinner idea).
 *  Discrete shapes read as intended at ~8 frames/s, where a ring turned in
 *  steps looked jerky and a smooth one redrew the host window every refresh
 *  (INC-2026-0001). The CSS module steps through them, no JS timer. */
const GLYPHS = ['·', '✢', '✳', '✶', '✻', '✽', '✻', '✶', '✳', '✢']

export function ActivityIndicator({ visible, text, thinking, color }: ActivityIndicatorProps): JSX.Element {
  const liveText = useLiveText(text, visible)
  // CSS-module class still owns the spinner geometry/animation; we only
  // override the glyph colour inline so the indicator picks up the
  // active tab's accent (red/pink/orange/etc.) instead of the always-
  // blue --accent-primary fallback.
  const spinnerStyle = color ? `color:${color}` : undefined
  return (
    <div class={`${styles.indicator}${visible ? ` ${styles.visible}` : ''}`}>
      <span class={styles.spinner} style={spinnerStyle} aria-hidden="true">
        <span class={styles.frames}>
          {GLYPHS.map((g, i) => <span key={i}>{g}</span>)}
        </span>
      </span>
      <span class={styles.text}>{liveText}</span>
      {thinking && <span class={styles.thinking} data-tooltip={thinking}>· {truncateThinking(thinking)}</span>}
    </div>
  )
}
