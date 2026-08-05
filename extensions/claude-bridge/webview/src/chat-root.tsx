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
import { installExternalLinkHandler } from './lib/external-links'
import { ChatRoot } from './roots/ChatRoot'
import { installCommandApi } from './lib/command-api'

applyInitialTheme()
installHljsThemeSwapper()
installExternalLinkHandler()
// `window.bridgeCmd` — scripted access to the same functions the buttons call.
// Installed after `./bridge-shim`, which populates `window.kaminBridge`.
installCommandApi(window.kaminBridge)

const root = document.getElementById('root') ?? (() => {
  const d = document.createElement('div'); d.id = 'root'; document.body.appendChild(d); return d
})()
// Chat = MainPanel (grows) + optional plan/todos aside, laid out in a row.
root.style.cssText = 'display:flex;flex-direction:row;height:100vh;width:100%;overflow:hidden'
render(<ChatRoot />, root)
notifyHostReady()
