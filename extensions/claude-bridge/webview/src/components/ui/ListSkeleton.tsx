import type { JSX } from 'preact'
import styles from './ListSkeleton.module.css'

interface Props {
  /** How many placeholder rows to render (defaults to 6). */
  rows?: number
  /** Optional shimmer for a top toolbar/search bar. */
  toolbar?: boolean
}

/** Shimmer skeleton for a list-shaped panel while its first data is loading.
 *  Used so a cold-open panel reads as "loading" instead of showing an empty
 *  "None found" that's indistinguishable from a genuinely empty result. */
export function ListSkeleton({ rows = 6, toolbar = false }: Props): JSX.Element {
  const widths = [0.9, 0.7, 0.82, 0.6, 0.75, 0.5, 0.68, 0.8]
  return (
    <div class={styles.wrap} role="status" aria-label="Loading…">
      {toolbar && <span class={`${styles.sk} ${styles.toolbar}`} aria-hidden="true" />}
      {Array.from({ length: rows }, (_, i) => {
        const w = widths[i % widths.length]!
        return (
          <div key={i} class={styles.row} aria-hidden="true">
            <span class={`${styles.sk} ${styles.icon}`} />
            <div class={styles.lines}>
              <span class={`${styles.sk} ${styles.line}`} style={`width:${Math.round(w * 100)}%`} />
              <span class={`${styles.sk} ${styles.lineDim}`} style={`width:${Math.round(w * 60)}%`} />
            </div>
          </div>
        )
      })}
    </div>
  )
}
