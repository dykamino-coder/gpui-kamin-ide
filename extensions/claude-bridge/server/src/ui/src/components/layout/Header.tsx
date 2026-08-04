import { signal } from '@preact/signals'
import { theme, toggleTheme } from '../../signals/ui'
import { api } from '../../services/api-client'
import styles from './Header.module.css'

const installerAvailable = signal(false)

// Check whether the KaminIDE installer is present on the server (findInstaller
// prefers the KaminIDE setup.exe) so we only show the download button when it
// can actually be fetched.
fetch('/api/download/check')
  .then(r => r.json())
  .then(d => { installerAvailable.value = d.available })
  .catch(() => {})

const ASCII_BANNER = `\
 ██████╗ ██████╗ ███████╗███╗   ██╗     ██████╗██╗      █████╗ ██╗   ██╗██████╗ ███████╗    ███╗   ███╗ █████╗ ██╗  ██╗    ██████╗ ██████╗ ██╗██████╗  ██████╗ ███████╗
██╔═══██╗██╔══██╗██╔════╝████╗  ██║    ██╔════╝██║     ██╔══██╗██║   ██║██╔══██╗██╔════╝    ████╗ ████║██╔══██╗╚██╗██╔╝    ██╔══██╗██╔══██╗██║██╔══██╗██╔════╝ ██╔════╝
██║   ██║██████╔╝█████╗  ██╔██╗ ██║    ██║     ██║     ███████║██║   ██║██║  ██║█████╗      ██╔████╔██║███████║ ╚███╔╝     ██████╔╝██████╔╝██║██║  ██║██║  ███╗█████╗
██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║    ██║     ██║     ██╔══██║██║   ██║██║  ██║██╔══╝      ██║╚██╔╝██║██╔══██║ ██╔██╗     ██╔══██╗██╔══██╗██║██║  ██║██║   ██║██╔══╝
╚██████╔╝██║     ███████╗██║ ╚████║    ╚██████╗███████╗██║  ██║╚██████╔╝██████╔╝███████╗    ██║ ╚═╝ ██║██║  ██║██╔╝ ██╗    ██████╔╝██║  ██║██║██████╔╝╚██████╔╝███████╗
 ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝     ╚═════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝    ╚═╝     ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝    ╚═════╝ ╚═╝  ╚═╝╚═╝╚═════╝  ╚═════╝ ╚══════╝`

function handleClearHistory() {
  if (!confirm('Clear all request history? This cannot be undone.')) return
  api.clearHistory().then(() => {
    api.getStats().catch(() => {})
  }).catch(() => {})
}

export function Header() {
  return (
    <header class={styles.header}>
      <div class={styles.left}>
        <pre class={styles.ascii}>{ASCII_BANNER}</pre>
      </div>
      <div class={styles.right}>
        {installerAvailable.value && (
          <a href="/download" target="_blank" rel="noopener" class={styles.downloadBtn} title="Download KaminIDE (Windows)">
            <i class="fa-solid fa-download" />
            <span>KaminIDE</span>
          </a>
        )}
        <button class={styles.actionBtn} onClick={handleClearHistory} title="Clear all request history">
          <i class="fa-solid fa-trash" />
        </button>
        <button class={styles.actionBtn} onClick={toggleTheme} title={theme.value === 'dark' ? 'Light theme' : 'Dark theme'}>
          <i class={`fa-solid ${theme.value === 'dark' ? 'fa-moon' : 'fa-sun'}`} />
        </button>
      </div>
    </header>
  )
}
