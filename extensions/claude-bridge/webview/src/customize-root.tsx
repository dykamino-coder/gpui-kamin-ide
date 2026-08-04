import { render } from 'preact'
import '@xterm/xterm/css/xterm.css'
import '@fortawesome/fontawesome-free/css/all.min.css'
import './theme/global.css'
import './theme/dark-theme.css'
import './theme/light-theme.css'
import './theme/legacy-global.css'
import './theme/components/sidebar.css'
import './theme/components/chat.css'
import './theme/components/jsonl.css'
import './theme/components/widgets.css'
import './theme/webview-overrides.css'
import './theme/kamin-bridge.css'
import './bridge-shim'
import { applyInitialTheme } from './theme/apply-theme'
import { installHljsThemeSwapper } from './theme/highlight-theme'
import { notifyHostReady } from './lib/host-ready'
import { CustomizeRoot } from './roots/CustomizeRoot'

applyInitialTheme()
installHljsThemeSwapper()

const root = document.getElementById('root') ?? (() => {
  const d = document.createElement('div'); d.id = 'root'; document.body.appendChild(d); return d
})()
root.style.cssText = 'height:100vh;width:100%;overflow:hidden'
render(<CustomizeRoot />, root)
notifyHostReady()
