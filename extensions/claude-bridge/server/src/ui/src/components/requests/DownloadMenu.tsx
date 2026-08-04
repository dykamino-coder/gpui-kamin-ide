import { useState, useRef, useEffect } from 'preact/hooks'
import type { RequestLogEntry } from '../../types'
import { downloadRequest, downloadRequestHtml } from './download'
import styles from './DownloadMenu.module.css'

interface Props {
  entry: RequestLogEntry
  btnClass?: string
}

export function DownloadMenu({ entry, btnClass }: Props) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [open])

  return (
    <div class={styles.wrap} ref={ref}>
      <button
        class={btnClass || styles.dlBtn}
        onClick={() => setOpen(!open)}
        title="Download"
      >
        <i class="fa-solid fa-download" />
      </button>
      {open && (
        <div class={styles.menu}>
          <button class={styles.menuItem} onClick={() => { downloadRequest(entry); setOpen(false) }}>
            <i class="fa-solid fa-file-lines" /> TXT
          </button>
          <button class={styles.menuItem} onClick={() => { downloadRequestHtml(entry); setOpen(false) }}>
            <i class="fa-solid fa-file-code" /> HTML
          </button>
        </div>
      )}
    </div>
  )
}
