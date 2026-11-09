import { render } from 'preact'
import '@xterm/xterm/css/xterm.css'
import '@fortawesome/fontawesome-free/css/all.min.css'
import '../theme/global.css'
import '../theme/dark-theme.css'
import '../theme/light-theme.css'
import '../theme/legacy-global.css'
import '../theme/components/sidebar.css'
import '../theme/components/chat.css'
import '../theme/components/jsonl.css'
import '../theme/components/widgets.css'
import '../bridge-shim'
import { App } from '../App'
import { applyInitialTheme } from '../theme/apply-theme'
import { installHljsThemeSwapper } from '../theme/highlight-theme'
import { notifyHostReady } from '../lib/host-ready'
import { installExternalLinkHandler } from '../lib/external-links'
import { installCommandApi } from '../lib/command-api'

export function mount(root: HTMLElement): void {
  applyInitialTheme()
  installHljsThemeSwapper()
  installExternalLinkHandler()
  installCommandApi(window.kaminBridge)
  render(<App />, root)
  notifyHostReady()
}
