import type { JSX } from 'preact'
import { useRef, useEffect } from 'preact/hooks'
import styles from './WidgetBlock.module.css'

/** The ShowWidget tool rendered as a CLEAN inline visual — no tool-block chrome
 *  (no header/name/chevron/Result footer), no border, no scrollbar. Just the
 *  widget, style-isolated in a Shadow DOM (an iframe is blocked by the webview
 *  CSP; `innerHTML` never runs `<script>`, so the widget stays declarative
 *  HTML/CSS/SVG). Natural height — it grows with its content. */
export function WidgetBlock({ input }: { input: unknown }): JSX.Element | null {
  const i = (input ?? {}) as { html?: string; title?: string; __streaming?: boolean }
  const html = typeof i.html === 'string' ? i.html : ''
  const title = typeof i.title === 'string' ? i.title : ''

  const hostRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    const el = hostRef.current
    if (!el || !html) return
    const root = el.shadowRoot ?? el.attachShadow({ mode: 'open' })
    root.innerHTML =
      '<style>:host{display:block;color:#e6e6ee;font-family:system-ui,-apple-system,sans-serif;font-size:13px;line-height:1.5}'
      + 'img,svg,canvas,video,table{max-width:100%}a{color:#8ab4f8}</style>'
      + html
  }, [html])

  // While the tool input is still streaming in, `html` isn't parsed yet.
  if (i.__streaming === true || !html) {
    return (
      <div class={styles.widget}>
        <div class={styles.loading}><i class="fas fa-circle-notch fa-spin" /> Rendering widget…</div>
      </div>
    )
  }
  return (
    <div class={styles.widget}>
      {title && <div class={styles.caption}>{title}</div>}
      <div ref={hostRef} class={styles.host} />
    </div>
  )
}
